use lexopt::prelude::*;
use std::ffi::OsString;

fn main() -> Result<(), lexopt::Error> {
    let (mut first, mut json, mut tilde) = (false, false, std::env::var_os("DV_EXPAND_TILDE").is_some());
    let mut names: Vec<OsString> = Vec::new();
    let mut p = lexopt::Parser::from_env();
    while let Some(arg) = p.next()? {
        match arg {
            Short('1') | Long("first") => first = true,
            Long("json") => json = true,
            Long("expand-tilde") => tilde = true,
            Value(v) => names.push(v),
            _ => return Err(arg.unexpected()),
        }
    }
    if names.is_empty() {
        eprintln!("usage: dv [-1] [--json] NAME...");
        std::process::exit(2);
    }
    std::process::exit(if first || json || tilde { 0 } else { names.len() as i32 * 0 });
}
