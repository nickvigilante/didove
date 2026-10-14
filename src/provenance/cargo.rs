//! `cargo install` puts binaries in `$CARGO_HOME/bin` and records them in `$CARGO_HOME/.crates2.json`.
//!
//! The file looks like:
//!
//! ```json
//! {"installs": {"ripgrep 14.1.0 (registry+https://github.com/rust-lang/crates.io-index)":
//!               {"bins": ["rg"], "...": "..."}}}
//! ```
//!
//! The key is `<crate> <version> (<source>)` and `bins` lists the executables it installed.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Detector, Provenance};

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[derive(Deserialize)]
struct Crates2 {
    installs: BTreeMap<String, Install>,
}

#[derive(Deserialize)]
struct Install {
    #[serde(default)]
    bins: Vec<String>,
}

pub struct Cargo {
    bin_dir: PathBuf,
    crates2: PathBuf,
    /// Parsed once, on first use. `None` inside means missing or unreadable.
    installs: OnceCell<Option<Crates2>>,
}

impl Cargo {
    pub fn new(cargo_home: PathBuf) -> Self {
        // Compare against the real directory, because the resolved path is always canonical.
        let bin = cargo_home.join("bin");
        Cargo {
            bin_dir: fs::canonicalize(&bin).unwrap_or(bin),
            crates2: cargo_home.join(".crates2.json"),
            installs: OnceCell::new(),
        }
    }

    fn installs(&self) -> Option<&Crates2> {
        self.installs
            .get_or_init(|| serde_json::from_slice(&fs::read(&self.crates2).ok()?).ok())
            .as_ref()
    }
}

impl Detector for Cargo {
    fn detect(&self, resolved: &Path) -> Option<Provenance> {
        if resolved.parent()? != self.bin_dir {
            return None;
        }
        let bin = resolved.file_name().and_then(OsStr::to_str)?;
        let (key, _) = self
            .installs()?
            .installs
            .iter()
            .find(|(_, i)| i.bins.iter().any(|b| b == bin))?;
        parse_key(key)
    }
}

/// `"name 1.2.3 (source)"` into a [`Provenance`].
fn parse_key(key: &str) -> Option<Provenance> {
    let (name, rest) = key.split_once(' ')?;
    let (version, source) = rest.split_once(' ')?;
    let source = source.strip_prefix('(')?.strip_suffix(')')?;
    Some(Provenance {
        source: "cargo",
        package: name.to_owned(),
        version: Some(version.to_owned()),
        detail: (source != CRATES_IO).then(|| source.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_home(json: &str) -> (tempfile::TempDir, Cargo) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join(".crates2.json"), json).unwrap();
        let cargo = Cargo::new(dir.path().to_path_buf());
        (dir, cargo)
    }

    const JSON: &str = r#"{"installs": {
        "ripgrep 14.1.0 (registry+https://github.com/rust-lang/crates.io-index)": {"bins": ["rg"]},
        "mytool 0.3.0 (git+https://github.com/me/mytool#abc123)": {"bins": ["mytool", "mytool-helper"]},
        "local 0.1.0 (path+file:///home/me/local)": {"bins": ["local"]}
    }}"#;

    fn bin(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        fs::canonicalize(dir.path().join("bin")).unwrap().join(name)
    }

    #[test]
    fn registry_crate() {
        let (dir, cargo) = fake_home(JSON);
        let p = cargo.detect(&bin(&dir, "rg")).unwrap();
        assert_eq!(p.source, "cargo");
        assert_eq!(p.package, "ripgrep");
        assert_eq!(p.version.as_deref(), Some("14.1.0"));
        assert_eq!(
            p.detail, None,
            "crates.io is the default and needs no detail"
        );
    }

    #[test]
    fn git_and_path_sources_are_kept_as_detail() {
        let (dir, cargo) = fake_home(JSON);
        let git = cargo.detect(&bin(&dir, "mytool-helper")).unwrap();
        assert_eq!(git.package, "mytool");
        assert_eq!(
            git.detail.as_deref(),
            Some("git+https://github.com/me/mytool#abc123")
        );
        let local = cargo.detect(&bin(&dir, "local")).unwrap();
        assert_eq!(local.detail.as_deref(), Some("path+file:///home/me/local"));
    }

    #[test]
    fn unlisted_binary_and_other_directories_are_ignored() {
        let (dir, cargo) = fake_home(JSON);
        assert_eq!(cargo.detect(&bin(&dir, "rustup")), None);
        assert_eq!(cargo.detect(Path::new("/usr/bin/rg")), None);
    }

    #[test]
    fn missing_or_corrupt_crates2_is_silent() {
        let (dir, cargo) = fake_home("not json");
        assert_eq!(cargo.detect(&bin(&dir, "rg")), None);
        fs::remove_file(dir.path().join(".crates2.json")).unwrap();
        let cargo = Cargo::new(dir.path().to_path_buf());
        assert_eq!(cargo.detect(&bin(&dir, "rg")), None);
    }
}
