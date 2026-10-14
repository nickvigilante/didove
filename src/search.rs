//! Walk PATH and record, for every entry, whether it produced a runnable match and why not.
//!
//! The walk never runs anything and never trims, deduplicates or reorders entries.
//! See `docs/decisions/0001-path-search.md` for why this is hand-rolled rather than the `which` crate.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{Access, AtFlags, CWD, accessat};
use serde::Serialize;

/// What happened at one PATH entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A regular file we may execute (checked with `faccessat(X_OK, AT_EACCESS)`).
    Executable,
    /// A regular file, but not executable by us.
    NotExecutable,
    /// Something exists under the name, but it is not a regular file (a directory, say).
    NotAFile,
    /// A symlink whose target does not exist.
    BrokenSymlink,
    /// The PATH entry's directory does not exist.
    /// Only reported when diagnosing; otherwise this case is [`Outcome::NoSuchFile`].
    DirMissing,
    /// A component of the PATH entry is a file, not a directory.
    NotADirectory,
    /// The directory exists (or was not examined) and holds nothing under that name.
    NoSuchFile,
    /// Any other I/O failure, such as an untraversable directory.
    Error(io::ErrorKind),
}

impl Outcome {
    /// The stable name used in JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Executable => "executable",
            Outcome::NotExecutable => "not_executable",
            Outcome::NotAFile => "not_a_file",
            Outcome::BrokenSymlink => "broken_symlink",
            Outcome::DirMissing => "dir_missing",
            Outcome::NotADirectory => "not_a_directory",
            Outcome::NoSuchFile => "no_such_file",
            Outcome::Error(_) => "error",
        }
    }

    /// Whether something answering to the name exists here but cannot run.
    /// These are worth telling the user about.
    pub fn is_unrunnable_candidate(self) -> bool {
        matches!(
            self,
            Outcome::NotExecutable | Outcome::NotAFile | Outcome::BrokenSymlink
        )
    }
}

/// The result of probing one PATH entry for one name.
#[derive(Debug, Clone)]
pub struct Probe {
    /// Zero-based position in PATH, or `None` when the name contained a `/` and PATH was not used.
    pub index: Option<usize>,
    /// The PATH entry exactly as written (possibly empty or relative).
    pub entry: PathBuf,
    /// The absolute path that was tested.
    pub candidate: PathBuf,
    pub outcome: Outcome,
    /// For an executable match: the PATH index of an earlier match that is the same file on disk.
    pub same_file_as: Option<usize>,
}

impl Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// Knobs for [`search_path`].
#[derive(Debug, Clone)]
pub struct SearchOptions<'a> {
    /// Used for empty and relative PATH entries. Must be absolute.
    pub cwd: &'a Path,
    /// Home directory, used only when `expand_tilde` is set.
    pub home: Option<&'a Path>,
    /// Expand a leading `~` in PATH entries, as bash does and `execvp` does not.
    pub expand_tilde: bool,
    /// Spend extra syscalls telling "missing directory" from "missing file".
    pub diagnose: bool,
    /// Stop at the first executable match.
    pub stop_at_first: bool,
}

/// Search every PATH entry for `name`.
///
/// A `name` containing `/` is not searched for on PATH: it is tested as a single path, like a shell would.
/// An empty `name` matches nothing.
pub fn search_path(
    name: &std::ffi::OsStr,
    path: &std::ffi::OsStr,
    opts: &SearchOptions,
) -> Vec<Probe> {
    if name.is_empty() {
        return Vec::new();
    }
    let mut seen: Vec<(FileId, usize)> = Vec::new();

    if name.as_encoded_bytes().contains(&b'/') {
        let candidate = absolutize(Path::new(name), opts.cwd);
        let (outcome, _) = classify(&candidate, opts.diagnose);
        return vec![Probe {
            index: None,
            entry: candidate
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            candidate,
            outcome,
            same_file_as: None,
        }];
    }

    let mut probes = Vec::new();
    for (index, entry) in std::env::split_paths(path).enumerate() {
        let dir = resolve_entry(&entry, opts);
        let candidate = dir.join(name);
        let (outcome, id) = classify(&candidate, opts.diagnose);
        let mut same_file_as = None;
        if let (Outcome::Executable, Some(id)) = (outcome, id) {
            same_file_as = seen.iter().find(|(s, _)| *s == id).map(|(_, i)| *i);
            seen.push((id, index));
        }
        probes.push(Probe {
            index: Some(index),
            entry,
            candidate,
            outcome,
            same_file_as,
        });
        if opts.stop_at_first && outcome == Outcome::Executable {
            break;
        }
    }
    probes
}

type FileId = (u64, u64);

/// Turn a PATH entry into an absolute directory.
fn resolve_entry(entry: &Path, opts: &SearchOptions) -> PathBuf {
    if entry.as_os_str().is_empty() {
        return opts.cwd.to_path_buf();
    }
    if opts.expand_tilde
        && let Some(home) = opts.home
        && let Ok(rest) = entry.strip_prefix("~")
    {
        return absolutize(&home.join(rest), opts.cwd);
    }
    absolutize(entry, opts.cwd)
}

/// Join onto `cwd` (absolute paths replace it) and drop `.` components.
fn absolutize(path: &Path, cwd: &Path) -> PathBuf {
    cwd.join(path)
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect()
}

/// Decide what the file at `candidate` is, using as few syscalls as the answer allows.
fn classify(candidate: &Path, diagnose: bool) -> (Outcome, Option<FileId>) {
    let lstat = match fs::symlink_metadata(candidate) {
        Ok(md) => md,
        Err(e) => return (classify_miss(candidate, &e, diagnose), None),
    };
    // Only a symlink needs a second look to find what it points at.
    let md = if lstat.file_type().is_symlink() {
        match fs::metadata(candidate) {
            Ok(md) => md,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Outcome::BrokenSymlink, None),
            Err(e) => return (Outcome::Error(e.kind()), None),
        }
    } else {
        lstat
    };
    if !md.is_file() {
        return (Outcome::NotAFile, None);
    }
    match accessat(CWD, candidate, Access::EXEC_OK, AtFlags::EACCESS) {
        Ok(()) => (Outcome::Executable, Some((md.dev(), md.ino()))),
        Err(_) => (Outcome::NotExecutable, None),
    }
}

fn classify_miss(candidate: &Path, err: &io::Error, diagnose: bool) -> Outcome {
    match err.kind() {
        io::ErrorKind::NotADirectory => Outcome::NotADirectory,
        io::ErrorKind::NotFound if diagnose => match candidate.parent().map(fs::metadata) {
            Some(Err(e)) if e.kind() == io::ErrorKind::NotFound => Outcome::DirMissing,
            _ => Outcome::NoSuchFile,
        },
        io::ErrorKind::NotFound => Outcome::NoSuchFile,
        kind => Outcome::Error(kind),
    }
}
