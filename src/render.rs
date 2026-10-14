//! Text and JSON output.
//!
//! The JSON shape is a public contract, documented in `docs/json-output.md`.
//! Bump [`JSON_VERSION`] if a field is removed or changes meaning.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::Path;

use anstyle::{AnsiColor, Style};
use serde::Serialize;

use crate::chain::Hop;
use crate::lookup::{Match, NameResult, Skipped};
use crate::provenance::Provenance;
use crate::search::Outcome;

pub const JSON_VERSION: u32 = 1;

const WINNER: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)));
const SHADOWED: Style = Style::new().dimmed();
const SOURCE: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan)));
const WARN: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));
const BROKEN: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)));

/// Print each found name's matches. `w` should strip colour when output is not a terminal.
///
/// With several names, each gets a header line and its entries are indented under it.
pub fn render_text<W: Write>(w: &mut W, results: &[NameResult]) -> io::Result<()> {
    let headers = results.len() > 1;
    for r in results
        .iter()
        .filter(|r| r.found() || !r.skipped.is_empty())
    {
        let indent = if headers {
            writeln!(w, "{}", safe(&r.name))?;
            "  "
        } else {
            ""
        };
        for m in &r.matches {
            render_match(w, m, indent)?;
        }
        for s in &r.skipped {
            render_skipped(w, s, indent)?;
        }
    }
    Ok(())
}

fn render_match<W: Write>(w: &mut W, m: &Match, indent: &str) -> io::Result<()> {
    let path = safe(m.path.as_os_str());
    if m.winner {
        writeln!(w, "{indent}{WINNER}* {path}{WINNER:#}")?;
    } else {
        let why = match m.duplicate_of {
            Some(_) => "shadowed, same file as an earlier match",
            None => "shadowed",
        };
        writeln!(w, "{indent}  {SHADOWED}{path} ({why}){SHADOWED:#}")?;
    }
    if m.duplicate_of.is_some() {
        return Ok(()); // the earlier match already shows the chain and package
    }
    let detail_indent = format!("{indent}    ");
    if let Some(chain) = &m.chain {
        for hop in &chain.hops {
            render_hop(w, hop, &detail_indent)?;
        }
    }
    if let Some(p) = &m.provenance {
        writeln!(w, "{detail_indent}{SOURCE}{}{SOURCE:#}", describe(p))?;
    }
    Ok(())
}

fn render_hop<W: Write>(w: &mut W, hop: &Hop, indent: &str) -> io::Result<()> {
    if hop.broken {
        writeln!(
            w,
            "{indent}{BROKEN}-> {} (broken: target does not exist){BROKEN:#}",
            safe(hop.resolved.as_os_str())
        )
    } else {
        writeln!(w, "{indent}-> {}", safe(hop.resolved.as_os_str()))
    }
}

fn render_skipped<W: Write>(w: &mut W, s: &Skipped, indent: &str) -> io::Result<()> {
    let why = reason_text(s.reason);
    writeln!(
        w,
        "{indent}{WARN}! {} ({why}, skipped){WARN:#}",
        safe(s.path.as_os_str())
    )
}

fn reason_text(o: Outcome) -> &'static str {
    match o {
        Outcome::NotExecutable => "not executable",
        Outcome::NotAFile => "not a file",
        Outcome::BrokenSymlink => "broken symlink",
        _ => "unusable",
    }
}

/// `homebrew: ripgrep 14.1.1`, with any detail in parentheses.
fn describe(p: &Provenance) -> String {
    let mut s = format!("{}: {}", p.source, safe(OsStr::new(&p.package)));
    if let Some(v) = &p.version {
        s.push(' ');
        s.push_str(&safe(OsStr::new(v)));
    }
    if let Some(d) = &p.detail {
        s.push_str(&format!(" ({})", safe(OsStr::new(d))));
    }
    s
}

/// Make untrusted text safe to print to a terminal.
///
/// File names, symlink targets and package metadata are controlled by whoever can write to
/// a PATH directory or a package, and may contain escape sequences that rewrite the screen
/// or set the window title. Control characters become visible escapes such as `\u{1b}`.
/// Invalid UTF-8 becomes U+FFFD, as it does in the JSON output.
pub fn safe(s: &OsStr) -> Cow<'_, str> {
    let text = s.to_string_lossy();
    if !text.chars().any(char::is_control) {
        return text;
    }
    Cow::Owned(
        text.chars()
            .flat_map(|c| {
                let escaped: Box<dyn Iterator<Item = char>> = if c.is_control() {
                    Box::new(c.escape_default())
                } else {
                    Box::new(std::iter::once(c))
                };
                escaped
            })
            .collect(),
    )
}

// ---- JSON ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_escapes_control_characters_and_leaves_the_rest() {
        assert_eq!(safe(OsStr::new("/usr/bin/tool")), "/usr/bin/tool");
        assert_eq!(
            safe(OsStr::new("/opt/caf\u{e9}/\u{65e5}\u{672c}")),
            "/opt/caf\u{e9}/\u{65e5}\u{672c}"
        );
        assert_eq!(safe(OsStr::new("a\x1b[31mb")), "a\\u{1b}[31mb");
        assert_eq!(safe(OsStr::new("a\x07b\nc\u{9b}")), "a\\u{7}b\\nc\\u{9b}");
    }
}

#[derive(Serialize)]
struct JsonOutput {
    version: u32,
    results: Vec<JsonResult>,
}

#[derive(Serialize)]
struct JsonResult {
    name: String,
    found: bool,
    matches: Vec<JsonMatch>,
    skipped: Vec<JsonSkipped>,
}

#[derive(Serialize)]
struct JsonMatch {
    path: String,
    path_index: Option<usize>,
    winner: bool,
    duplicate_of: Option<usize>,
    symlink_chain: Vec<JsonHop>,
    real_path: Option<String>,
    provenance: Option<Provenance>,
}

#[derive(Serialize)]
struct JsonHop {
    path: String,
    target: String,
    resolved: String,
    broken: bool,
}

#[derive(Serialize)]
struct JsonSkipped {
    path: String,
    path_index: Option<usize>,
    reason: Outcome,
}

/// Paths are not always UTF-8; JSON strings must be, so invalid bytes become U+FFFD.
fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub fn render_json<W: Write>(w: &mut W, results: &[NameResult]) -> io::Result<()> {
    let out = JsonOutput {
        version: JSON_VERSION,
        results: results
            .iter()
            .map(|r| JsonResult {
                name: r.name.to_string_lossy().into_owned(),
                found: r.found(),
                matches: r
                    .matches
                    .iter()
                    .map(|m| JsonMatch {
                        path: s(&m.path),
                        path_index: m.path_index,
                        winner: m.winner,
                        duplicate_of: m.duplicate_of,
                        symlink_chain: m
                            .chain
                            .as_ref()
                            .map(|c| {
                                c.hops
                                    .iter()
                                    .map(|h| JsonHop {
                                        path: s(&h.path),
                                        target: s(&h.target),
                                        resolved: s(&h.resolved),
                                        broken: h.broken,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        real_path: m.chain.as_ref().and_then(|c| c.real_path.as_deref()).map(s),
                        provenance: m.provenance.clone(),
                    })
                    .collect(),
                skipped: r
                    .skipped
                    .iter()
                    .map(|k| JsonSkipped {
                        path: s(&k.path),
                        path_index: k.path_index,
                        reason: k.reason,
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_writer_pretty(&mut *w, &out)?;
    writeln!(w)
}
