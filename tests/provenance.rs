//! End to end: fake package-manager trees, a real PATH walk, and the detectors reading them.

mod common;

use std::fs;

use common::Fx;
use didove::lookup::{Env, Options, lookup};
use didove::provenance::{DetectorEnv, default_detectors};

const OPTS: Options = Options {
    expand_tilde: false,
    winner_only: false,
    detail: true,
};

fn run(
    fx: &Fx,
    path: std::ffi::OsString,
    name: &str,
    denv: &DetectorEnv,
) -> didove::lookup::NameResult {
    let env = Env {
        path,
        cwd: fx.cwd(),
        home: None,
    };
    lookup(
        std::ffi::OsStr::new(name),
        &env,
        OPTS,
        &default_detectors(denv),
    )
}

fn denv(fx: &Fx) -> DetectorEnv {
    DetectorEnv {
        cargo_home: Some(fx.p("cargo")),
        nix_store: fx.p("nix/store"),
    }
}

fn summary(m: &didove::lookup::Match) -> Option<(String, String, Option<String>)> {
    m.provenance
        .as_ref()
        .map(|p| (p.source.to_string(), p.package.clone(), p.version.clone()))
}

#[test]
fn homebrew_through_a_prefix_symlink() {
    // Both layouts Homebrew uses: `<prefix>/bin/x` -> `../Cellar/x/<ver>/bin/x`.
    for prefix in ["opt/homebrew", "usr/local"] {
        let fx = Fx::new();
        fx.exe(&format!("{prefix}/Cellar/ripgrep/14.1.1/bin/rg"));
        fx.link(
            &format!("{prefix}/bin/rg"),
            "../Cellar/ripgrep/14.1.1/bin/rg",
        );
        let r = run(
            &fx,
            fx.path([fx.p(&format!("{prefix}/bin"))]),
            "rg",
            &denv(&fx),
        );
        assert_eq!(
            summary(&r.matches[0]),
            Some(("homebrew".into(), "ripgrep".into(), Some("14.1.1".into()))),
            "{prefix}"
        );
    }
}

#[test]
fn cargo_install_via_crates2_json() {
    let fx = Fx::new();
    fx.exe("cargo/bin/rg");
    fx.exe("cargo/bin/rustup"); // present on disk but not in .crates2.json
    fs::write(
        fx.p("cargo/.crates2.json"),
        r#"{"installs":{"ripgrep 14.1.0 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["rg"]}}}"#,
    )
    .unwrap();
    let path = fx.path([fx.p("cargo/bin")]);
    let r = run(&fx, path.clone(), "rg", &denv(&fx));
    assert_eq!(
        summary(&r.matches[0]),
        Some(("cargo".into(), "ripgrep".into(), Some("14.1.0".into())))
    );
    let r = run(&fx, path, "rustup", &denv(&fx));
    assert_eq!(summary(&r.matches[0]), None);
}

#[test]
fn nix_store_path_behind_a_profile_symlink() {
    let fx = Fx::new();
    let hash = "q3v5z1ck0r8dn5lqhxk6w4i6zbjm7iqx";
    fx.exe(&format!("nix/store/{hash}-ripgrep-14.1.0/bin/rg"));
    fx.link(
        "profile/bin/rg",
        fx.p(&format!("nix/store/{hash}-ripgrep-14.1.0/bin/rg")),
    );
    let r = run(&fx, fx.path([fx.p("profile/bin")]), "rg", &denv(&fx));
    assert_eq!(
        summary(&r.matches[0]),
        Some(("nix".into(), "ripgrep".into(), Some("14.1.0".into())))
    );
}

#[test]
fn each_shadowed_match_gets_its_own_provenance() {
    let fx = Fx::new();
    let hash = "q3v5z1ck0r8dn5lqhxk6w4i6zbjm7iqx";
    fx.exe("cargo/bin/rg");
    fs::write(
        fx.p("cargo/.crates2.json"),
        r#"{"installs":{"ripgrep 14.0.3 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["rg"]}}}"#,
    )
    .unwrap();
    fx.exe("opt/homebrew/Cellar/ripgrep/14.1.1/bin/rg");
    fx.link("opt/homebrew/bin/rg", "../Cellar/ripgrep/14.1.1/bin/rg");
    fx.exe(&format!("nix/store/{hash}-ripgrep-13.0.0/bin/rg"));
    fx.link(
        "nixbin/rg",
        fx.p(&format!("nix/store/{hash}-ripgrep-13.0.0/bin/rg")),
    );
    fx.exe("usr/bin/rg");
    let path = fx.path([
        fx.p("opt/homebrew/bin"),
        fx.p("cargo/bin"),
        fx.p("nixbin"),
        fx.p("usr/bin"),
    ]);
    let r = run(&fx, path, "rg", &denv(&fx));
    let got: Vec<_> = r
        .matches
        .iter()
        .map(|m| (m.winner, summary(m).map(|s| (s.0, s.2.unwrap()))))
        .collect();
    assert_eq!(
        got,
        [
            (true, Some(("homebrew".into(), "14.1.1".into()))),
            (false, Some(("cargo".into(), "14.0.3".into()))),
            (false, Some(("nix".into(), "13.0.0".into()))),
            (false, None),
        ]
    );
}

#[test]
fn a_broken_link_into_the_nix_store_is_skipped_not_matched() {
    // Not runnable, so it is reported as a skipped candidate; a missing store path is not a crash.
    let fx = Fx::new();
    fx.link(
        "bin/rg",
        fx.p("nix/store/q3v5z1ck0r8dn5lqhxk6w4i6zbjm7iqx-ripgrep-14.1.0/bin/rg"),
    );
    let r = run(&fx, fx.path([fx.p("bin")]), "rg", &denv(&fx));
    assert!(!r.found());
    assert_eq!(r.skipped.len(), 1);
}
