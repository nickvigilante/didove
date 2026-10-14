//! Where a file came from, according to whatever package manager put it there.
//!
//! A [`Detector`] is handed a resolved path and either recognises it or says nothing.
//! Detectors only read files; none of them runs the program they are asked about.
//! Cheap detectors (these three) run for every match; expensive ones (dpkg, rpm) belong
//! behind `--deep` or on the winning match only.

pub mod cargo;
pub mod homebrew;
pub mod nix;

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A package that owns a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    /// Short name of the source: `homebrew`, `cargo`, `nix`.
    pub source: &'static str,
    pub package: String,
    pub version: Option<String>,
    /// Anything source-specific worth showing, such as a git URL for a crate installed from git.
    pub detail: Option<String>,
}

pub trait Detector {
    /// `resolved` is the real path of a match, or where a broken symlink chain was heading.
    /// It may not exist on disk, so detectors must not require that.
    fn detect(&self, resolved: &Path) -> Option<Provenance>;
}

/// Locations the detectors need that are not part of the matched path.
#[derive(Debug, Clone)]
pub struct DetectorEnv {
    /// `$CARGO_HOME`, or `~/.cargo`. `None` if neither can be determined.
    pub cargo_home: Option<PathBuf>,
    /// The Nix store, normally `/nix/store`.
    pub nix_store: PathBuf,
}

impl DetectorEnv {
    pub fn from_env() -> Self {
        let cargo_home = std::env::var_os("CARGO_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")));
        DetectorEnv {
            cargo_home,
            nix_store: PathBuf::from("/nix/store"),
        }
    }
}

/// The detectors `dv` ships with, in the order they are tried.
pub fn default_detectors(env: &DetectorEnv) -> Vec<Box<dyn Detector>> {
    let mut detectors: Vec<Box<dyn Detector>> = vec![Box::new(homebrew::Homebrew)];
    if let Some(home) = &env.cargo_home {
        detectors.push(Box::new(cargo::Cargo::new(home.clone())));
    }
    detectors.push(Box::new(nix::Nix::new(env.nix_store.clone())));
    detectors
}

/// The first detector that recognises `resolved`.
pub fn detect(detectors: &[Box<dyn Detector>], resolved: &Path) -> Option<Provenance> {
    detectors.iter().find_map(|d| d.detect(resolved))
}
