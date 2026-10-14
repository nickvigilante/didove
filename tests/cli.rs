mod common;

use std::fs;
use std::process::Output;

use assert_cmd::Command;
use common::Fx;
use insta::assert_snapshot;

/// A `dv` with a minimal, deterministic environment rooted in the fixture.
fn dv(fx: &Fx, path: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.env_clear()
        .env("PATH", path)
        .env("HOME", fx.p("home"))
        .env("CARGO_HOME", fx.p("cargo"))
        .current_dir(fx.cwd());
    cmd
}

fn stdout(fx: &Fx, o: &Output) -> String {
    fx.redact(&String::from_utf8(o.stdout.clone()).unwrap())
}

fn stderr(fx: &Fx, o: &Output) -> String {
    fx.redact(&String::from_utf8(o.stderr.clone()).unwrap())
}

/// One name, three PATH entries: a Homebrew-style symlink wins, a plain binary is shadowed,
/// and a file without the execute bit is reported as skipped.
fn rg_fixture() -> (Fx, std::ffi::OsString) {
    let fx = Fx::new();
    fx.exe("opt/homebrew/Cellar/ripgrep/14.1.1/bin/rg");
    fx.link("opt/homebrew/bin/rg", "../Cellar/ripgrep/14.1.1/bin/rg");
    fx.file("noexec/rg", 0o644);
    fx.exe("usr/bin/rg");
    let path = fx.path([fx.p("opt/homebrew/bin"), fx.p("noexec"), fx.p("usr/bin")]);
    (fx, path)
}

#[test]
fn lists_winner_shadowed_and_skipped() {
    let (fx, path) = rg_fixture();
    let out = dv(&fx, path)
        .arg("rg")
        .assert()
        .success()
        .get_output()
        .clone();
    assert_snapshot!(stdout(&fx, &out));
    assert_eq!(stderr(&fx, &out), "");
}

#[test]
fn first_prints_only_the_winning_path() {
    let (fx, path) = rg_fixture();
    let out = dv(&fx, path)
        .args(["-1", "rg"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(stdout(&fx, &out), "[ROOT]/opt/homebrew/bin/rg\n");
    let out = dv(&fx, fx.path([fx.p("usr/bin")]))
        .args(["--first", "rg"])
        .output()
        .unwrap();
    assert_eq!(stdout(&fx, &out), "[ROOT]/usr/bin/rg\n");
}

#[test]
fn first_with_several_names_prints_one_line_each() {
    let fx = Fx::new();
    fx.exe("bin/one");
    fx.exe("bin/two");
    let out = dv(&fx, fx.path([fx.p("bin")]))
        .args(["-1", "one", "two"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(stdout(&fx, &out), "[ROOT]/bin/one\n[ROOT]/bin/two\n");
}

#[test]
fn exit_status_is_1_when_any_name_is_missing_and_found_names_still_print() {
    let fx = Fx::new();
    fx.exe("bin/one");
    let out = dv(&fx, fx.path([fx.p("bin")]))
        .args(["one", "nope"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    assert_snapshot!(stdout(&fx, &out));
    assert_eq!(stderr(&fx, &out), "dv: nope: not found in PATH\n");
}

#[test]
fn missing_name_with_first_prints_nothing_to_stdout() {
    let fx = Fx::new();
    fx.dir("bin");
    let out = dv(&fx, fx.path([fx.p("bin")]))
        .args(["-1", "nope"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    assert_eq!(stdout(&fx, &out), "");
    assert_eq!(stderr(&fx, &out), "dv: nope: not found in PATH\n");
}

#[test]
fn usage_errors_exit_2() {
    let fx = Fx::new();
    dv(&fx, "").assert().code(2); // no NAME
    dv(&fx, "").arg("--no-such-flag").assert().code(2);
}

#[test]
fn json_output() {
    let (fx, path) = rg_fixture();
    let out = dv(&fx, path)
        .args(["--json", "rg", "nope"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = stdout(&fx, &out);
    assert_snapshot!(text);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["version"], 1);
    assert_eq!(v["results"][0]["matches"][0]["winner"], true);
    assert_eq!(v["results"][1]["found"], false);
}

#[test]
fn json_with_first_keeps_only_the_winner_but_still_has_detail() {
    let (fx, path) = rg_fixture();
    let out = dv(&fx, path)
        .args(["--json", "-1", "rg"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let matches = v["results"][0]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["provenance"]["source"], "homebrew");
}

#[test]
fn broken_symlinks_are_reported_as_skipped() {
    let fx = Fx::new();
    fx.link("a/tool", fx.p("gone/tool"));
    fx.exe("b/tool");
    let out = dv(&fx, fx.path([fx.p("a"), fx.p("b")]))
        .arg("tool")
        .assert()
        .success()
        .get_output()
        .clone();
    assert_snapshot!(stdout(&fx, &out));
}

#[test]
fn output_is_plain_when_piped_and_coloured_when_forced() {
    let (fx, path) = rg_fixture();
    let plain = dv(&fx, path.clone()).arg("rg").output().unwrap();
    assert!(
        !plain.stdout.contains(&0x1b),
        "no escape codes when stdout is not a terminal"
    );

    let forced = dv(&fx, path.clone())
        .arg("rg")
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    assert!(
        forced.stdout.contains(&0x1b),
        "CLICOLOR_FORCE=1 turns colour on"
    );

    let no_color = dv(&fx, path)
        .arg("rg")
        .env("CLICOLOR_FORCE", "1")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        !no_color.stdout.contains(&0x1b),
        "NO_COLOR wins over CLICOLOR_FORCE"
    );
}

#[test]
fn json_and_first_are_never_coloured() {
    let (fx, path) = rg_fixture();
    for args in [&["--json", "rg"][..], &["-1", "rg"][..]] {
        let out = dv(&fx, path.clone())
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap();
        assert!(!out.stdout.contains(&0x1b), "{args:?}");
    }
}

#[test]
fn tilde_is_literal_by_default_and_expands_on_request() {
    let fx = Fx::new();
    fx.exe("home/bin/tool");
    dv(&fx, "~/bin").arg("tool").assert().code(1);
    dv(&fx, "~/bin")
        .args(["--expand-tilde", "tool"])
        .assert()
        .success();
    dv(&fx, "~/bin")
        .env("DV_EXPAND_TILDE", "1")
        .arg("tool")
        .assert()
        .success();
}

#[test]
fn a_program_named_init_is_just_a_name() {
    // There are no subcommands, so `init`, `doctor` and `help` can all be looked up.
    let fx = Fx::new();
    fx.exe("bin/init");
    fx.exe("bin/doctor");
    for name in ["init", "doctor"] {
        let out = dv(&fx, fx.path([fx.p("bin")]))
            .arg(name)
            .assert()
            .success()
            .get_output()
            .clone();
        assert_eq!(stdout(&fx, &out), format!("* [ROOT]/bin/{name}\n"));
    }
    dv(&fx, fx.path([fx.p("bin")]))
        .args(["--", "init"])
        .assert()
        .success();
}

#[test]
fn names_containing_a_slash_are_paths_not_searches() {
    let fx = Fx::new();
    fx.exe("cwd/sub/tool");
    let out = dv(&fx, "/nonexistent")
        .args(["-1", "sub/tool"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(stdout(&fx, &out), "[ROOT]/cwd/sub/tool\n");
}

#[test]
fn cargo_install_is_recognised_end_to_end() {
    let fx = Fx::new();
    fx.exe("cargo/bin/rg");
    fs::write(
        fx.p("cargo/.crates2.json"),
        r#"{"installs":{"ripgrep 14.1.0 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["rg"]}}}"#,
    )
    .unwrap();
    let out = dv(&fx, fx.path([fx.p("cargo/bin")]))
        .arg("rg")
        .assert()
        .success()
        .get_output()
        .clone();
    assert_snapshot!(stdout(&fx, &out));
}

#[test]
fn unset_path_finds_nothing_and_exits_1() {
    let fx = Fx::new();
    let mut cmd = Command::cargo_bin("dv").unwrap();
    cmd.env_clear()
        .current_dir(fx.cwd())
        .arg("tool")
        .assert()
        .code(1);
}

#[test]
fn expand_tilde_env_accepts_the_usual_spellings() {
    let fx = Fx::new();
    fx.exe("home/bin/tool");
    for yes in ["1", "true", "yes", "on"] {
        dv(&fx, "~/bin")
            .env("DV_EXPAND_TILDE", yes)
            .arg("tool")
            .assert()
            .success();
    }
    for no in ["0", "false", "no", "off"] {
        dv(&fx, "~/bin")
            .env("DV_EXPAND_TILDE", no)
            .arg("tool")
            .assert()
            .code(1);
    }
}
