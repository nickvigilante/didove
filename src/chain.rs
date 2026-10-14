//! Follow a symlink one hop at a time, down to the real file.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

/// Linux's own limit is 40 hops (`ELOOP`); matching it keeps loop detection predictable.
const MAX_HOPS: usize = 40;

/// One symlink in a chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hop {
    /// The symlink itself.
    pub path: PathBuf,
    /// What `readlink` returned, exactly as stored.
    pub target: PathBuf,
    /// The absolute path the link points at.
    pub resolved: PathBuf,
    /// Nothing exists at `resolved`.
    pub broken: bool,
}

/// How a chain ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    /// The last hop is a real (non-symlink) file, or the start was not a symlink at all.
    File,
    /// A link points at something that does not exist.
    Broken,
    /// More than [`MAX_HOPS`] hops: a cycle, or an absurdly long chain.
    Loop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    /// Symlinks in order. Empty when the start is not a symlink.
    pub hops: Vec<Hop>,
    /// The fully resolved path, with every directory symlink resolved too. `None` unless `end` is `File`.
    pub real_path: Option<PathBuf>,
    pub end: End,
}

impl Chain {
    /// The best available path for provenance: the real file, or where a broken chain was heading.
    pub fn final_path<'a>(&'a self, start: &'a Path) -> &'a Path {
        match (&self.real_path, self.hops.last()) {
            (Some(p), _) => p,
            (None, Some(h)) => &h.resolved,
            (None, None) => start,
        }
    }
}

/// Follow `start` (an absolute path) through its symlinks.
pub fn resolve(start: &Path) -> Chain {
    let mut hops: Vec<Hop> = Vec::new();
    let mut cur = start.to_path_buf();
    loop {
        let Ok(md) = fs::symlink_metadata(&cur) else {
            // Only reachable for the starting path; later hops are checked before we step onto them.
            return Chain {
                hops,
                real_path: None,
                end: End::Broken,
            };
        };
        if !md.file_type().is_symlink() {
            let real_path = fs::canonicalize(&cur).ok();
            return Chain {
                hops,
                real_path,
                end: End::File,
            };
        }
        if hops.len() >= MAX_HOPS {
            return Chain {
                hops,
                real_path: None,
                end: End::Loop,
            };
        }
        let Ok(target) = fs::read_link(&cur) else {
            return Chain {
                hops,
                real_path: None,
                end: End::Broken,
            };
        };
        let resolved = resolve_target(&cur, &target);
        let broken = fs::symlink_metadata(&resolved).is_err();
        hops.push(Hop {
            path: cur,
            target,
            resolved: resolved.clone(),
            broken,
        });
        if broken {
            return Chain {
                hops,
                real_path: None,
                end: End::Broken,
            };
        }
        cur = resolved;
    }
}

/// Where does `link` point? Resolves every directory in the way, but leaves the final
/// component alone so that it can be followed as the next hop.
fn resolve_target(link: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        link.parent().unwrap_or(Path::new("/")).join(target)
    };
    match (joined.parent(), joined.file_name()) {
        (Some(dir), Some(name)) => match fs::canonicalize(dir) {
            Ok(dir) => dir.join(name),
            Err(_) => lexical(&joined),
        },
        _ => lexical(&joined),
    }
}

/// Collapse `.` and `..` without touching the filesystem. Only a fallback, for a missing directory.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}
