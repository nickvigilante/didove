mod common;

use std::ffi::OsStr;

use common::Fx;
use didove::search::{Outcome, Probe, SearchOptions, search_path};

fn opts(fx: &Fx) -> SearchOptions<'static> {
    // Leak the cwd so the options can be `'static`; these are short-lived test processes.
    let cwd: &'static std::path::Path = Box::leak(fx.cwd().into_boxed_path());
    SearchOptions {
        cwd,
        home: None,
        expand_tilde: false,
        diagnose: false,
        stop_at_first: false,
    }
}

fn run(path: &OsStr, o: &SearchOptions) -> Vec<Probe> {
    search_path(OsStr::new("tool"), path, o)
}

fn outcomes(probes: &[Probe]) -> Vec<Outcome> {
    probes.iter().map(|p| p.outcome).collect()
}

fn matches(probes: &[Probe]) -> Vec<&std::path::Path> {
    probes
        .iter()
        .filter(|p| p.outcome == Outcome::Executable)
        .map(|p| p.candidate.as_path())
        .collect()
}

#[test]
fn non_executable_file_before_executable_one() {
    let fx = Fx::new();
    fx.file("a/tool", 0o644);
    let b = fx.exe("b/tool");
    let probes = run(&fx.path([fx.p("a"), fx.p("b")]), &opts(&fx));
    assert_eq!(
        outcomes(&probes),
        [Outcome::NotExecutable, Outcome::Executable]
    );
    assert_eq!(matches(&probes), [b.as_path()]);
}

#[test]
fn exec_bit_that_is_not_ours_does_not_count() {
    // Mode 0011: executable by group and other, but we are the owner, so the owner bits apply.
    // A mode-bit test (`mode & 0o111`) says yes; `faccessat` says no.
    let fx = Fx::new();
    fx.file("a/tool", 0o011);
    fx.exe("b/tool");
    let probes = run(&fx.path([fx.p("a"), fx.p("b")]), &opts(&fx));
    assert_eq!(
        outcomes(&probes),
        [Outcome::NotExecutable, Outcome::Executable]
    );
}

#[test]
fn directory_with_the_commands_name() {
    let fx = Fx::new();
    fx.dir("a/tool");
    fx.exe("b/tool");
    let probes = run(&fx.path([fx.p("a"), fx.p("b")]), &opts(&fx));
    assert_eq!(outcomes(&probes), [Outcome::NotAFile, Outcome::Executable]);
}

#[test]
fn symlinks_to_non_executable_to_nowhere_and_to_executable() {
    let fx = Fx::new();
    let plain = fx.file("real/plain", 0o644);
    fx.link("a/tool", &plain);
    fx.link("b/tool", fx.p("nowhere"));
    let real = fx.exe("real/tool-1.2");
    fx.link("c/tool", &real);
    let probes = run(&fx.path([fx.p("a"), fx.p("b"), fx.p("c")]), &opts(&fx));
    assert_eq!(
        outcomes(&probes),
        [
            Outcome::NotExecutable,
            Outcome::BrokenSymlink,
            Outcome::Executable
        ]
    );
    // The match is reported at the symlink's own path, not the target's.
    assert_eq!(probes[2].candidate, fx.p("c/tool"));
}

#[test]
fn empty_dot_and_relative_entries_resolve_against_cwd() {
    let fx = Fx::new();
    fx.exe("cwd/tool");
    fx.exe("cwd/rel/bin/tool");
    let path = fx.path(["", ".", "rel/bin"].map(std::path::PathBuf::from));
    let probes = run(&path, &opts(&fx));
    assert_eq!(
        matches(&probes),
        [fx.p("cwd/tool"), fx.p("cwd/tool"), fx.p("cwd/rel/bin/tool")]
    );
    assert!(
        probes[0].entry.as_os_str().is_empty(),
        "the entry is kept as written"
    );
    assert_eq!(probes[1].same_file_as, Some(0));
}

#[test]
fn directory_name_with_a_trailing_space_is_not_trimmed() {
    let fx = Fx::new();
    let tool = fx.exe("spacey /tool");
    let mut dir = fx.p("spacey ").into_os_string();
    assert!(dir.to_str().unwrap().ends_with(' '));
    dir.push(""); // keep as is
    let probes = run(&fx.path([dir]), &opts(&fx));
    assert_eq!(matches(&probes), [tool.as_path()]);
}

#[test]
fn duplicates_are_kept_and_flagged() {
    let fx = Fx::new();
    fx.exe("a/tool");
    fx.dir("usr");
    fx.exe("usr/bin/tool");
    fx.link("bin", fx.p("usr/bin"));
    let path = fx.path([fx.p("a"), fx.p("a"), fx.p("bin"), fx.p("usr/bin")]);
    let probes = run(&path, &opts(&fx));
    let flags: Vec<_> = probes.iter().map(|p| p.same_file_as).collect();
    assert_eq!(flags, [None, Some(0), None, Some(2)]);
}

#[test]
fn missing_directory_missing_file_and_file_in_the_way() {
    let fx = Fx::new();
    fx.dir("empty");
    fx.exe("notdir"); // a file where a directory should be
    let path = fx.path([fx.p("missing"), fx.p("empty"), fx.p("notdir")]);

    let mut o = opts(&fx);
    o.diagnose = true;
    let probes = run(&path, &o);
    assert_eq!(
        outcomes(&probes),
        [
            Outcome::DirMissing,
            Outcome::NoSuchFile,
            Outcome::NotADirectory
        ]
    );

    // Without diagnosing, a missing directory is indistinguishable from a missing file,
    // but the file-in-the-way case comes free with the error.
    o.diagnose = false;
    let probes = run(&path, &o);
    assert_eq!(
        outcomes(&probes),
        [
            Outcome::NoSuchFile,
            Outcome::NoSuchFile,
            Outcome::NotADirectory
        ]
    );
}

#[test]
fn tilde_is_literal_unless_asked() {
    let fx = Fx::new();
    let tool = fx.exe("home/bin/tool");
    let path = fx.path(["~/bin"]);
    let home = fx.p("home");

    let mut o = opts(&fx);
    o.home = Some(&home);
    assert!(matches(&run(&path, &o)).is_empty());

    o.expand_tilde = true;
    assert_eq!(matches(&run(&path, &o)), [tool.as_path()]);

    // `~user` and a `~` that is not the first component are never expanded.
    let path = fx.path(["~other/bin", "x/~/bin"]);
    assert!(matches(&run(&path, &o)).is_empty());
}

#[test]
fn names_with_a_slash_skip_path() {
    let fx = Fx::new();
    let tool = fx.exe("cwd/sub/tool");
    let o = opts(&fx);
    let probes = search_path(OsStr::new("sub/tool"), OsStr::new("/nonexistent"), &o);
    assert_eq!(probes.len(), 1);
    assert_eq!(probes[0].index, None);
    assert_eq!(
        (probes[0].outcome, &probes[0].candidate),
        (Outcome::Executable, &tool)
    );
}

#[test]
fn empty_name_matches_nothing() {
    let fx = Fx::new();
    fx.exe("a/tool");
    assert!(search_path(OsStr::new(""), &fx.path([fx.p("a")]), &opts(&fx)).is_empty());
}

#[test]
fn stop_at_first_stops_after_the_winner() {
    let fx = Fx::new();
    fx.exe("a/tool");
    fx.exe("b/tool");
    let mut o = opts(&fx);
    o.stop_at_first = true;
    let probes = run(&fx.path([fx.p("a"), fx.p("b")]), &o);
    assert_eq!(probes.len(), 1);
}
