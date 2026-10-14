//! One name in, one [`NameResult`] out: search, then resolve symlinks, then detect provenance.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::chain::{self, Chain};
use crate::provenance::{self, Detector, Provenance};
use crate::search::{self, Outcome, Probe, SearchOptions};

/// Everything a lookup reads from the environment, gathered once so tests can fake it.
#[derive(Debug, Clone)]
pub struct Env {
    pub path: OsString,
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> std::io::Result<Self> {
        Ok(Env {
            path: std::env::var_os("PATH").unwrap_or_default(),
            cwd: std::env::current_dir()?,
            home: std::env::var_os("HOME").map(PathBuf::from),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub expand_tilde: bool,
    /// Do the least work that finds the winner: stop at the first match, skip symlink and provenance work.
    pub winner_only: bool,
    /// With `winner_only`, still resolve symlinks and detect provenance for the winner (used by `--json`).
    pub detail: bool,
}

/// An executable found on PATH.
#[derive(Debug, Clone)]
pub struct Match {
    pub path: PathBuf,
    /// `None` when the name was a path rather than a command name.
    pub path_index: Option<usize>,
    /// The first match is the one that runs.
    pub winner: bool,
    /// Index into [`NameResult::matches`] of an earlier match that is the same file.
    pub duplicate_of: Option<usize>,
    pub chain: Option<Chain>,
    pub provenance: Option<Provenance>,
}

/// Something that answers to the name but cannot run.
#[derive(Debug, Clone)]
pub struct Skipped {
    pub path: PathBuf,
    pub path_index: Option<usize>,
    pub reason: Outcome,
}

#[derive(Debug, Clone)]
pub struct NameResult {
    pub name: OsString,
    pub matches: Vec<Match>,
    pub skipped: Vec<Skipped>,
    /// Every PATH entry visited, with its outcome. Kept for `--doctor`.
    pub probes: Vec<Probe>,
}

impl NameResult {
    pub fn found(&self) -> bool {
        !self.matches.is_empty()
    }
}

pub fn lookup(
    name: &OsStr,
    env: &Env,
    opts: Options,
    detectors: &[Box<dyn Detector>],
) -> NameResult {
    let probes = search::search_path(
        name,
        &env.path,
        &SearchOptions {
            cwd: &env.cwd,
            home: env.home.as_deref(),
            expand_tilde: opts.expand_tilde,
            diagnose: false,
            stop_at_first: opts.winner_only,
        },
    );
    let with_detail = !opts.winner_only || opts.detail;

    let mut matches: Vec<Match> = Vec::new();
    let mut skipped = Vec::new();
    for probe in &probes {
        if probe.outcome == Outcome::Executable {
            let chain = with_detail.then(|| chain::resolve(&probe.candidate));
            let provenance = chain
                .as_ref()
                .and_then(|c| provenance::detect(detectors, c.final_path(&probe.candidate)));
            let duplicate_of = probe
                .same_file_as
                .and_then(|idx| matches.iter().position(|m| m.path_index == Some(idx)));
            matches.push(Match {
                path: probe.candidate.clone(),
                path_index: probe.index,
                winner: matches.is_empty(),
                duplicate_of,
                chain,
                provenance,
            });
        } else if probe.outcome.is_unrunnable_candidate() {
            skipped.push(Skipped {
                path: probe.candidate.clone(),
                path_index: probe.index,
                reason: probe.outcome,
            });
        }
    }
    NameResult {
        name: name.to_owned(),
        matches,
        skipped,
        probes,
    }
}
