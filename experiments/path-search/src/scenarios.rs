//! Edge-case fixtures, run against both approaches.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::{Outcome, Probe, naive_all, walk, which_all};

pub struct Row {
    pub name: &'static str,
    pub walk: Vec<String>,
    pub walk_probes: Vec<Probe>,
    pub which: Vec<String>,
    pub which_errors: Vec<String>,
    pub naive: Vec<String>,
}

struct Fx {
    root: tempfile::TempDir,
}

impl Fx {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("cwd")).unwrap();
        Fx { root }
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.root.path().join(rel)
    }
    fn cwd(&self) -> PathBuf {
        self.p("cwd")
    }
    fn file(&self, rel: &str, mode: u32) -> PathBuf {
        let p = self.p(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        p
    }
    fn dir(&self, rel: &str) -> PathBuf {
        let p = self.p(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn join(&self, entries: &[&OsStr]) -> OsString {
        std::env::join_paths(entries).unwrap()
    }
    /// Render a path with the fixture root replaced by `$T`.
    fn show(&self, p: &Path) -> String {
        let s = p
            .to_string_lossy()
            .replace(&*self.root.path().to_string_lossy(), "$T");
        if s.is_empty() { "(empty)".into() } else { s }
    }
    fn row(&self, name: &'static str, path: &OsStr) -> Row {
        let walk_probes = walk(OsStr::new("tool"), path, &self.cwd());
        let walked = walk_probes
            .iter()
            .filter(|p| p.outcome == Outcome::Executable)
            .map(|p| self.show(&p.candidate))
            .collect();
        let w = which_all(OsStr::new("tool"), path, &self.cwd());
        Row {
            name,
            walk: walked,
            walk_probes,
            which: w.matches.iter().map(|p| self.show(p)).collect(),
            which_errors: w.nonfatal_errors,
            naive: naive_all(OsStr::new("tool"), path, &self.cwd())
                .iter()
                .map(|p| self.show(p))
                .collect(),
        }
    }
}

pub fn run_all() -> Vec<Row> {
    let mut rows = Vec::new();

    // 1. A non-executable file earlier on PATH than an executable one.
    {
        let f = Fx::new();
        f.file("a/tool", 0o644);
        f.file("b/tool", 0o755);
        let path = f.join(&[f.p("a").as_os_str(), f.p("b").as_os_str()]);
        rows.push(f.row("non-executable file before executable one", &path));
    }
    // 2. Execute bit set, but not for the owner (us): mode 0o011.
    //    A bit-test says "executable"; access(X_OK) says no.
    {
        let f = Fx::new();
        f.file("a/tool", 0o011);
        f.file("b/tool", 0o755);
        let path = f.join(&[f.p("a").as_os_str(), f.p("b").as_os_str()]);
        rows.push(f.row("exec bit set for group/other only (we own it)", &path));
    }
    // 3. A directory with the command's name.
    {
        let f = Fx::new();
        f.dir("a/tool");
        f.file("b/tool", 0o755);
        let path = f.join(&[f.p("a").as_os_str(), f.p("b").as_os_str()]);
        rows.push(f.row("directory named like the command", &path));
    }
    // 4. Symlinks: to a non-executable file, broken, and to an executable.
    {
        let f = Fx::new();
        let plain = f.file("real/plain", 0o644);
        f.dir("a");
        symlink(&plain, f.p("a/tool")).unwrap();
        f.file("b/tool", 0o755);
        let path = f.join(&[f.p("a").as_os_str(), f.p("b").as_os_str()]);
        rows.push(f.row("symlink to non-executable file", &path));
    }
    {
        let f = Fx::new();
        f.dir("a");
        symlink(f.p("nowhere"), f.p("a/tool")).unwrap();
        f.file("b/tool", 0o755);
        let path = f.join(&[f.p("a").as_os_str(), f.p("b").as_os_str()]);
        rows.push(f.row("broken symlink", &path));
    }
    {
        let f = Fx::new();
        let real = f.file("real/tool-1.2", 0o755);
        f.dir("a");
        symlink(&real, f.p("a/tool")).unwrap();
        let path = f.join(&[f.p("a").as_os_str()]);
        rows.push(f.row("symlink to executable (returned path)", &path));
    }
    // 5. Empty entry, `.` entry, relative entry. Each has `tool` in a place only cwd reaches.
    {
        let f = Fx::new();
        f.file("cwd/tool", 0o755);
        f.file("b/tool", 0o755);
        let mut path = OsString::from(":");
        path.push(f.p("b"));
        rows.push(f.row("empty PATH entry (leading)", &path));
    }
    {
        let f = Fx::new();
        f.file("cwd/tool", 0o755);
        f.file("b/tool", 0o755);
        let path = f.join(&[OsStr::new("."), f.p("b").as_os_str()]);
        rows.push(f.row("`.` entry", &path));
    }
    {
        let f = Fx::new();
        f.file("cwd/rel/bin/tool", 0o755);
        let path = f.join(&[OsStr::new("rel/bin")]);
        rows.push(f.row("relative entry `rel/bin`", &path));
    }
    // 6. Trailing space in the directory name.
    {
        let f = Fx::new();
        f.file("spacey /tool", 0o755);
        let mut dir = f.p("spacey ").into_os_string();
        dir.push(""); // keep the trailing space
        let path = f.join(&[dir.as_os_str()]);
        rows.push(f.row("directory name ends in a space", &path));
    }
    // 7. Same directory twice.
    {
        let f = Fx::new();
        f.file("a/tool", 0o755);
        let path = f.join(&[f.p("a").as_os_str(), f.p("a").as_os_str()]);
        rows.push(f.row("same directory listed twice", &path));
    }
    // 8. Same file via a symlinked directory (merged-/usr style).
    {
        let f = Fx::new();
        f.file("usr/bin/tool", 0o755);
        symlink(f.p("usr/bin"), f.p("bin")).unwrap();
        let path = f.join(&[f.p("bin").as_os_str(), f.p("usr/bin").as_os_str()]);
        rows.push(f.row("same file via symlinked directory", &path));
    }
    // 9. Nonexistent directory first.
    {
        let f = Fx::new();
        f.file("b/tool", 0o755);
        let path = f.join(&[f.p("missing").as_os_str(), f.p("b").as_os_str()]);
        rows.push(f.row("nonexistent directory entry", &path));
    }
    rows
}
