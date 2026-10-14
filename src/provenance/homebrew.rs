//! Homebrew keeps every formula under `<prefix>/Cellar/<formula>/<version>/`.
//!
//! The prefix is `/opt/homebrew` on Apple silicon, `/usr/local` on Intel Macs and
//! `/home/linuxbrew/.linuxbrew` on Linux, but a custom prefix works too, so the match is
//! on the `Cellar` directory itself rather than on a fixed prefix.

use std::ffi::OsStr;
use std::path::{Component, Path};

use super::{Detector, Provenance};

pub struct Homebrew;

impl Detector for Homebrew {
    fn detect(&self, resolved: &Path) -> Option<Provenance> {
        let mut parts = resolved.components().filter_map(|c| match c {
            Component::Normal(s) => Some(s),
            _ => None,
        });
        parts.find(|c| *c == OsStr::new("Cellar"))?;
        let formula = parts.next()?;
        let version = parts.next()?;
        // Require something below the version directory: `Cellar/foo/1.0` alone is not a file of foo.
        parts.next()?;
        Some(Provenance {
            source: "homebrew",
            package: formula.to_str()?.to_owned(),
            version: Some(version.to_str()?.to_owned()),
            detail: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(p: &str) -> Option<Provenance> {
        Homebrew.detect(Path::new(p))
    }

    fn pv(p: &str) -> (String, Option<String>) {
        let found = detect(p).unwrap();
        (found.package, found.version)
    }

    #[test]
    fn apple_silicon_prefix() {
        assert_eq!(
            pv("/opt/homebrew/Cellar/ripgrep/14.1.1/bin/rg"),
            ("ripgrep".into(), Some("14.1.1".into()))
        );
    }

    #[test]
    fn intel_prefix_and_revision_suffix() {
        assert_eq!(
            pv("/usr/local/Cellar/openssl@3/3.3.1_1/bin/openssl"),
            ("openssl@3".into(), Some("3.3.1_1".into()))
        );
    }

    #[test]
    fn linuxbrew_and_custom_prefixes() {
        assert!(detect("/home/linuxbrew/.linuxbrew/Cellar/jq/1.7.1/bin/jq").is_some());
        assert!(detect("/Users/me/homebrew/Cellar/jq/1.7.1/bin/jq").is_some());
    }

    #[test]
    fn not_homebrew() {
        assert_eq!(detect("/usr/bin/rg"), None);
        assert_eq!(
            detect("/opt/homebrew/bin/rg"),
            None,
            "an unresolved symlink is not a Cellar path"
        );
    }

    #[test]
    fn needs_a_file_below_the_version() {
        assert_eq!(detect("/opt/homebrew/Cellar/ripgrep/14.1.1"), None);
        assert_eq!(detect("/opt/homebrew/Cellar/ripgrep"), None);
    }
}
