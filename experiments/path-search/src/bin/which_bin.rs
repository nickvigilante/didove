//! Benchmark target: the `which` crate, first match only.
use std::process::ExitCode;

fn main() -> ExitCode {
    let name = std::env::args_os().nth(1).unwrap_or_default();
    match which::which(name) {
        Ok(p) => {
            println!("{}", p.display());
            ExitCode::SUCCESS
        }
        Err(_) => ExitCode::FAILURE,
    }
}
