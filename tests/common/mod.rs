//! Shared fixture: a canonical temp directory with helpers for building fake filesystems.
#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

pub struct Fx {
    _dir: tempfile::TempDir,
    /// Canonical, so snapshots and `real_path` agree even where `/tmp` is a symlink.
    pub root: PathBuf,
}

impl Fx {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        fs::create_dir(root.join("cwd")).unwrap();
        Fx { _dir: dir, root }
    }

    pub fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn cwd(&self) -> PathBuf {
        self.p("cwd")
    }

    pub fn dir(&self, rel: &str) -> PathBuf {
        let p = self.p(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    pub fn file(&self, rel: &str, mode: u32) -> PathBuf {
        let p = self.p(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        p
    }

    pub fn exe(&self, rel: &str) -> PathBuf {
        self.file(rel, 0o755)
    }

    /// Create a symlink at `rel` pointing at `target` (stored verbatim).
    pub fn link(&self, rel: &str, target: impl AsRef<Path>) -> PathBuf {
        let p = self.p(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        symlink(target.as_ref(), &p).unwrap();
        p
    }

    /// Join entries into a PATH value.
    pub fn path<I, S>(&self, entries: I) -> OsString
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        std::env::join_paths(entries).unwrap()
    }

    /// Replace the fixture root with `[ROOT]` for stable snapshots.
    pub fn redact(&self, s: &str) -> String {
        s.replace(self.root.to_str().unwrap(), "[ROOT]")
    }
}
