//! Benchmark target: our own PATH walk, first match only.
use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    let name: OsString = std::env::args_os().nth(1).unwrap_or_default();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let cwd = std::env::current_dir().unwrap_or_default();
    match path_search::walk_matches(&name, &path, &cwd).first() {
        Some(p) => {
            println!("{}", p.display());
            ExitCode::SUCCESS
        }
        None => ExitCode::FAILURE,
    }
}
