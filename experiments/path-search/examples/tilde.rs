//! Does a literal `~/bin` PATH entry resolve? `execvp` says no; `which` may say yes.
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn main() {
    let home = tempfile_dir();
    fs::create_dir_all(home.join("bin")).unwrap();
    let tool = home.join("bin/tool");
    fs::write(&tool, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    // Single-threaded and before any other env access.
    unsafe { std::env::set_var("HOME", &home) };
    let path = OsStr::new("~/bin");
    let cwd = std::env::temp_dir();
    println!(
        "which : {:?}",
        path_search::which_all(OsStr::new("tool"), path, &cwd).matches
    );
    println!(
        "walk  : {:?}",
        path_search::walk_matches(OsStr::new("tool"), path, &cwd)
    );
}

fn tempfile_dir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("tilde-probe-{}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    p
}
