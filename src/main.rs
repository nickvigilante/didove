use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;
use didove::lookup::{self, Env, Options};
use didove::provenance::{DetectorEnv, default_detectors};
use didove::render;

/// Where did this command come from?
///
/// Lists every match for each NAME on PATH, in order.
/// The first is the one that runs; the rest are shadowed.
/// Each match shows its symlink chain and, if a package manager owns it, the package.
///
/// Exit status: 0 if every NAME was found, 1 if any was not, 2 on a usage error.
#[derive(Parser)]
#[command(name = "dv", version)]
struct Cli {
    /// Print only the path that runs, one per line, for use in scripts.
    #[arg(short = '1', long)]
    first: bool,

    /// Print results as JSON (see docs/json-output.md).
    #[arg(long)]
    json: bool,

    /// Expand a leading `~` in PATH entries, as bash does when it searches PATH.
    /// By default `~` is taken literally, as `execvp` and zsh do.
    #[arg(long, env = "DV_EXPAND_TILDE", value_parser = clap::builder::BoolishValueParser::new())]
    expand_tilde: bool,

    /// Command names to look up.
    #[arg(required = true, value_name = "NAME")]
    names: Vec<OsString>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dv: {e}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: &Cli) -> io::Result<ExitCode> {
    let env = Env::from_process()?;
    // `-1` without `--json` prints just the path, so skip building detectors it will not use.
    let detectors = if cli.first && !cli.json {
        Vec::new()
    } else {
        default_detectors(&DetectorEnv::from_env())
    };
    let opts = Options {
        expand_tilde: cli.expand_tilde,
        winner_only: cli.first,
        detail: cli.json,
    };

    let results: Vec<_> = cli
        .names
        .iter()
        .map(|name| lookup::lookup(name, &env, opts, &detectors))
        .collect();

    for r in results.iter().filter(|r| !r.found()) {
        eprintln!("dv: {}: not found in PATH", r.name.to_string_lossy());
    }

    if cli.json {
        let mut out = io::stdout().lock();
        render::render_json(&mut out, &results)?;
        out.flush()?;
    } else if cli.first {
        let mut out = io::stdout().lock();
        for m in results.iter().filter_map(|r| r.matches.first()) {
            writeln!(out, "{}", m.path.display())?;
        }
        out.flush()?;
    } else {
        let mut out = anstream::stdout().lock();
        render::render_text(&mut out, &results)?;
        out.flush()?;
    }

    Ok(if results.iter().all(|r| r.found()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
