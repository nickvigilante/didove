//! Nix store paths look like `/nix/store/<hash>-<name>-<version>[-<output>]/...`.
//!
//! The hash is 32 characters of Nix's base32 alphabet, which leaves out `e`, `o`, `u` and `t`.
//! The name and version split follows `builtins.parseDrvName`: at the first `-` that is
//! followed by something other than an ASCII letter.

use std::path::{Component, Path, PathBuf};

use super::{Detector, Provenance};

const HASH_LEN: usize = 32;
const BASE32: &str = "0123456789abcdfghijklmnpqrsvwxyz";

/// Names Nix appends for the non-default outputs of a derivation.
const OUTPUT_SUFFIXES: &[&str] = &[
    "bin", "dev", "lib", "man", "doc", "info", "out", "debug", "static",
];

pub struct Nix {
    store: PathBuf,
}

impl Nix {
    pub fn new(store: PathBuf) -> Self {
        Nix { store }
    }
}

impl Detector for Nix {
    fn detect(&self, resolved: &Path) -> Option<Provenance> {
        let rest = resolved.strip_prefix(&self.store).ok()?;
        let Some(Component::Normal(entry)) = rest.components().next() else {
            return None;
        };
        let entry = entry.to_str()?;
        let hash = entry.get(..HASH_LEN)?;
        let drv = entry.get(HASH_LEN..)?.strip_prefix('-')?;
        if !hash.chars().all(|c| BASE32.contains(c)) {
            return None;
        }
        let (name, version) = parse_drv_name(drv);
        Some(Provenance {
            source: "nix",
            package: name.to_owned(),
            version: version.map(strip_output_suffix).map(str::to_owned),
            detail: None,
        })
    }
}

/// Split `name-version` the way Nix does.
fn parse_drv_name(drv: &str) -> (&str, Option<&str>) {
    let bytes = drv.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'-' && !bytes[i + 1].is_ascii_alphabetic() {
            return (&drv[..i], Some(&drv[i + 1..]));
        }
    }
    (drv, None)
}

fn strip_output_suffix(version: &str) -> &str {
    for suffix in OUTPUT_SUFFIXES {
        if let Some(v) = version
            .strip_suffix(suffix)
            .and_then(|v| v.strip_suffix('-'))
        {
            return v;
        }
    }
    version
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "q3v5z1ck0r8dn5lqhxk6w4i6zbjm7iqx";

    fn detect(entry: &str) -> Option<Provenance> {
        let path = format!("/nix/store/{entry}/bin/tool");
        Nix::new("/nix/store".into()).detect(Path::new(&path))
    }

    fn pv(entry: &str) -> (String, Option<String>) {
        let p = detect(entry).unwrap();
        (p.package, p.version)
    }

    #[test]
    fn name_and_version() {
        assert_eq!(
            pv(&format!("{HASH}-ripgrep-14.1.0")),
            ("ripgrep".into(), Some("14.1.0".into()))
        );
        assert_eq!(
            pv(&format!("{HASH}-gnu-hello-2.12")),
            ("gnu-hello".into(), Some("2.12".into()))
        );
    }

    #[test]
    fn output_suffix_is_not_part_of_the_version() {
        assert_eq!(
            pv(&format!("{HASH}-jq-1.7.1-bin")),
            ("jq".into(), Some("1.7.1".into()))
        );
    }

    #[test]
    fn unstable_style_versions_follow_nix_parsing() {
        assert_eq!(
            pv(&format!("{HASH}-foo-unstable-2024-01-01")),
            ("foo-unstable".into(), Some("2024-01-01".into()))
        );
    }

    #[test]
    fn no_version() {
        assert_eq!(pv(&format!("{HASH}-wrapper")), ("wrapper".into(), None));
    }

    #[test]
    fn rejects_things_that_are_not_store_paths() {
        assert_eq!(detect("short-ripgrep-14.1.0"), None, "hash too short");
        assert_eq!(
            detect(&format!("{}-ripgrep-1.0", HASH.replace('q', "e"))),
            None,
            "'e' is not in the alphabet"
        );
        assert_eq!(
            detect(&format!("{HASH}ripgrep")),
            None,
            "no dash after the hash"
        );
        assert_eq!(
            Nix::new("/nix/store".into()).detect(Path::new("/usr/bin/rg")),
            None
        );
    }

    #[test]
    fn custom_store_location() {
        let nix = Nix::new("/tmp/fake-store".into());
        let path = format!("/tmp/fake-store/{HASH}-jq-1.7.1/bin/jq");
        assert_eq!(nix.detect(Path::new(&path)).unwrap().package, "jq");
    }
}
