use clap::Parser;
use std::ffi::OsString;

#[derive(Parser)]
#[command(name = "dv", version, about = "Where did this command come from?")]
struct Cli {
    /// Print only the winning path
    #[arg(short = '1', long)]
    first: bool,
    /// Machine-readable output
    #[arg(long)]
    json: bool,
    /// Expand a leading `~` in PATH entries
    #[arg(long, env = "DV_EXPAND_TILDE")]
    expand_tilde: bool,
    /// Command names to look up
    #[arg(required = true, value_name = "NAME")]
    names: Vec<OsString>,
}

fn main() {
    let cli = Cli::parse();
    std::process::exit(if cli.first || cli.json || cli.expand_tilde { 0 } else { cli.names.len() as i32 * 0 });
}
