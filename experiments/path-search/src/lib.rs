//! Two ways to search PATH, side by side, for the edge-case matrix.

#[cfg(feature = "scenarios")]
pub mod scenarios;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{Access, AtFlags, CWD, accessat};

/// Why a PATH entry did or did not produce a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A regular file we are allowed to execute.
    Executable,
    /// A regular file, but `faccessat(X_OK, AT_EACCESS)` said no.
    NotExecutable,
    /// The candidate exists but is not a regular file (a directory, say).
    NotAFile,
    /// The candidate is a symlink whose target does not exist.
    BrokenSymlink,
    /// Nothing at `entry/name` (missing file, or missing directory).
    Absent,
    /// Some other I/O error, such as an unreadable directory.
    Error(io::ErrorKind),
}

/// What happened at one PATH entry.
#[derive(Debug, Clone)]
pub struct Probe {
    /// Zero-based position in PATH.
    pub index: usize,
    /// The entry exactly as written (may be empty or relative).
    pub entry: PathBuf,
    /// The path that was tested, with empty and relative entries resolved against `cwd`.
    pub candidate: PathBuf,
    pub outcome: Outcome,
    /// For an executable match: the earlier match that is the same file on disk.
    pub same_file_as: Option<usize>,
}

/// Walk PATH ourselves, recording one `Probe` per entry.
///
/// `cwd` is explicit so tests need not change the process directory.
/// Entries are used verbatim: no trimming, no tilde expansion, no deduplication.
pub fn walk(name: &OsStr, path: &OsStr, cwd: &Path) -> Vec<Probe> {
    let mut probes = Vec::new();
    let mut seen: Vec<((u64, u64), usize)> = Vec::new();
    for (index, entry) in std::env::split_paths(path).enumerate() {
        let dir = if entry.as_os_str().is_empty() {
            cwd.to_path_buf()
        } else {
            cwd.join(&entry) // absolute entries replace cwd
        };
        let candidate: PathBuf = dir
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect::<PathBuf>()
            .join(name);
        let mut same_file_as = None;
        let outcome = match fs::metadata(&candidate) {
            Ok(md) if !md.is_file() => Outcome::NotAFile,
            Ok(md) => match accessat(CWD, &candidate, Access::EXEC_OK, AtFlags::EACCESS) {
                Ok(()) => {
                    let id = (md.dev(), md.ino());
                    same_file_as = seen.iter().find(|(s, _)| *s == id).map(|(_, i)| *i);
                    seen.push((id, index));
                    Outcome::Executable
                }
                Err(_) => Outcome::NotExecutable,
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if fs::symlink_metadata(&candidate).is_ok() {
                    Outcome::BrokenSymlink
                } else {
                    Outcome::Absent
                }
            }
            Err(e) => Outcome::Error(e.kind()),
        };
        probes.push(Probe {
            index,
            entry,
            candidate,
            outcome,
            same_file_as,
        });
    }
    probes
}

/// Convenience: the paths of every executable match from `walk`.
pub fn walk_matches(name: &OsStr, path: &OsStr, cwd: &Path) -> Vec<PathBuf> {
    walk(name, path, cwd)
        .into_iter()
        .filter(|p| p.outcome == Outcome::Executable)
        .map(|p| p.candidate)
        .collect()
}

/// Everything the `which` crate will tell us: matching paths, plus io errors with no path attached.
pub struct WhichResult {
    pub matches: Vec<PathBuf>,
    pub nonfatal_errors: Vec<String>,
}

pub fn which_all(name: &OsStr, path: &OsStr, cwd: &Path) -> WhichResult {
    let errors = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = errors.clone();
    let matches = which::WhichConfig::new()
        .binary_name(OsString::from(name))
        .custom_path_list(path.to_os_string())
        .custom_cwd(cwd.to_path_buf())
        .nonfatal_error_handler(move |e: which::NonFatalError| {
            sink.borrow_mut().push(e.to_string())
        })
        .all_results()
        .map(|it| it.collect())
        .unwrap_or_default();
    let nonfatal_errors = errors.borrow().clone();
    WhichResult {
        matches,
        nonfatal_errors,
    }
}

/// The `whence`-style control: any execute bit counts, and entries are trimmed.
/// It exists only to prove the edge-case tests can fail.
pub fn naive_all(name: &OsStr, path: &OsStr, cwd: &Path) -> Vec<PathBuf> {
    std::env::split_paths(path)
        .filter_map(|entry| {
            let trimmed = entry.to_string_lossy().trim().to_string();
            let dir = if trimmed.is_empty() {
                cwd.to_path_buf()
            } else {
                cwd.join(trimmed)
            };
            let candidate = dir.join(name);
            let md = fs::metadata(&candidate).ok()?;
            (md.is_file() && md.mode() & 0o111 != 0).then_some(candidate)
        })
        .collect()
}
