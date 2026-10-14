use path_search::Outcome;
use path_search::scenarios::{Row, run_all};

fn rows() -> Vec<Row> {
    run_all()
}

fn row<'a>(rows: &'a [Row], name: &str) -> &'a Row {
    rows.iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no scenario {name}"))
}

/// What every correct implementation must return for each scenario.
const EXPECTED: &[(&str, &[&str])] = &[
    ("non-executable file before executable one", &["$T/b/tool"]),
    (
        "exec bit set for group/other only (we own it)",
        &["$T/b/tool"],
    ),
    ("directory named like the command", &["$T/b/tool"]),
    ("symlink to non-executable file", &["$T/b/tool"]),
    ("broken symlink", &["$T/b/tool"]),
    ("symlink to executable (returned path)", &["$T/a/tool"]),
    ("empty PATH entry (leading)", &["$T/cwd/tool", "$T/b/tool"]),
    ("`.` entry", &["$T/cwd/tool", "$T/b/tool"]),
    ("relative entry `rel/bin`", &["$T/cwd/rel/bin/tool"]),
    ("directory name ends in a space", &["$T/spacey /tool"]),
    ("same directory listed twice", &["$T/a/tool", "$T/a/tool"]),
    (
        "same file via symlinked directory",
        &["$T/bin/tool", "$T/usr/bin/tool"],
    ),
    ("nonexistent directory entry", &["$T/b/tool"]),
];

#[test]
fn own_walker_matches_expected() {
    let rows = rows();
    for (name, want) in EXPECTED {
        assert_eq!(row(&rows, name).walk, *want, "walk: {name}");
    }
}

#[test]
fn which_crate_matches_expected() {
    let rows = rows();
    for (name, want) in EXPECTED {
        assert_eq!(row(&rows, name).which, *want, "which: {name}");
    }
}

/// The control must fail exactly where `whence` did, or these tests prove nothing.
#[test]
fn naive_control_is_caught() {
    let rows = rows();
    let wrong: Vec<&str> = EXPECTED
        .iter()
        .filter(|(name, want)| row(&rows, name).naive != *want)
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        wrong,
        [
            "exec bit set for group/other only (we own it)",
            "`.` entry", // cosmetic: `cwd/./tool`, not a different file
            "directory name ends in a space",
        ]
    );
}

#[test]
fn walker_records_why_each_entry_was_skipped() {
    let rows = rows();
    let outcomes = |name: &str| -> Vec<Outcome> {
        row(&rows, name)
            .walk_probes
            .iter()
            .map(|p| p.outcome.clone())
            .collect()
    };
    assert_eq!(
        outcomes("non-executable file before executable one"),
        [Outcome::NotExecutable, Outcome::Executable]
    );
    assert_eq!(
        outcomes("exec bit set for group/other only (we own it)"),
        [Outcome::NotExecutable, Outcome::Executable]
    );
    assert_eq!(
        outcomes("directory named like the command"),
        [Outcome::NotAFile, Outcome::Executable]
    );
    assert_eq!(
        outcomes("symlink to non-executable file"),
        [Outcome::NotExecutable, Outcome::Executable]
    );
    assert_eq!(
        outcomes("broken symlink"),
        [Outcome::BrokenSymlink, Outcome::Executable]
    );
    assert_eq!(
        outcomes("nonexistent directory entry"),
        [Outcome::Absent, Outcome::Executable]
    );
}

#[test]
fn walker_reports_position_and_duplicates() {
    let rows = rows();
    let twice = &row(&rows, "same directory listed twice").walk_probes;
    assert_eq!((twice[0].index, twice[1].index), (0, 1));
    assert_eq!(twice[0].same_file_as, None);
    assert_eq!(twice[1].same_file_as, Some(0));
    let via_link = &row(&rows, "same file via symlinked directory").walk_probes;
    assert_eq!(via_link[1].same_file_as, Some(0));
    let empty = &row(&rows, "empty PATH entry (leading)").walk_probes;
    assert!(empty[0].entry.as_os_str().is_empty());
}

/// `which` surfaces io errors but not which PATH entry produced them.
#[test]
fn which_errors_carry_no_path_or_position() {
    let rows = rows();
    let r = row(&rows, "non-executable file before executable one");
    assert_eq!(r.which_errors, ["Permission denied (os error 13)"]);
    // Silent for a directory in the way: no error is reported at all.
    assert!(
        row(&rows, "directory named like the command")
            .which_errors
            .is_empty()
    );
}

/// A file executable only by another user (root-owned, mode 0700).
///
/// Needs a fixture that only root can create, so `linux-bench.sh` builds it and runs this
/// test as an unprivileged user with `PATH_SEARCH_OTHERUSER_FIXTURE` pointing at it.
/// Layout: `a/tool` is root:root 0700, `b/tool` is root:root 0755.
/// Without the variable the test does nothing (and says so).
#[test]
fn file_executable_only_by_another_user() {
    use std::ffi::OsStr;
    use std::path::Path;

    let Some(fixture) = std::env::var_os("PATH_SEARCH_OTHERUSER_FIXTURE") else {
        eprintln!(
            "skipped: PATH_SEARCH_OTHERUSER_FIXTURE not set (needs root to build the fixture)"
        );
        return;
    };
    let fx = Path::new(&fixture);
    let path = std::env::join_paths([fx.join("a"), fx.join("b")]).unwrap();
    let name = OsStr::new("tool");
    let cwd = std::env::temp_dir();

    let probes = path_search::walk(name, &path, &cwd);
    assert_eq!(probes[0].outcome, Outcome::NotExecutable);
    assert_eq!(probes[1].outcome, Outcome::Executable);
    assert_eq!(
        path_search::which_all(name, &path, &cwd).matches,
        [fx.join("b/tool")]
    );
    // Root-owned 0700 has an exec bit, so a mode-bit check would pick the wrong file.
    assert_eq!(
        path_search::naive_all(name, &path, &cwd).first(),
        Some(&fx.join("a/tool"))
    );
}
