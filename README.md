# didove

_Di dove?_ is Italian for "from where?".
`didove` is a command-line tool that answers the question `which` can't: where did this command come from?
It installs one command, `dv`.

`which` tells you the first match on PATH and stops.
`dv` shows every match in order, marks the one that runs, follows each symlink down to the real file, and says which package manager put it there.

```console
$ dv rg
* /opt/homebrew/bin/rg
    -> /opt/homebrew/Cellar/ripgrep/14.1.1/bin/rg
    homebrew: ripgrep 14.1.1
  /Users/me/.cargo/bin/rg (shadowed)
    cargo: ripgrep 14.0.3
  /usr/local/bin/rg (shadowed)
! /opt/old/bin/rg (not executable, skipped)
```

## Usage

```text
dv [OPTIONS] NAME...
```

| Option | Effect |
| --- | --- |
| `-1`, `--first` | Print only the winning path, one per line, so `dv -1 foo` works like `which foo` in scripts. |
| `--json` | Print machine-readable output. The shape is documented in [docs/json-output.md](docs/json-output.md). |
| `--expand-tilde` | Expand a leading `~` in PATH entries, as bash does. Also set by `DV_EXPAND_TILDE=1`. |

Exit status matches `which`: 0 if every name was found, 1 if any was not, 2 on a usage error.
Output is plain when it is not a terminal, and colour respects `NO_COLOR`.
File names, symlink targets and package names can contain terminal escape sequences, so text output shows control characters as visible escapes such as `\u{1b}`.
With `-1`, the path is printed byte-for-byte when piped (for scripts) and escaped when stdout is a terminal.
JSON output escapes control characters as JSON requires.

There are no subcommands, so any name can be looked up, including one called `init` or `doctor`.
Future modes will be flags.

## What it recognises

| Source | How |
| --- | --- |
| Homebrew | The real path contains `Cellar/<formula>/<version>/`. |
| Cargo | The binary is in `$CARGO_HOME/bin` (default `~/.cargo/bin`) and `.crates2.json` lists it. |
| Nix | The real path is `/nix/store/<hash>-<name>-<version>/…`. |

`dv` never runs the programs it inspects.
Versions come from package metadata on disk, not from `--version`.

## Platforms

Linux and macOS.

## Building

```bash
cargo build --release
```

The minimum supported Rust version is 1.88.

## Design notes

- [ADR 0001: walk PATH ourselves instead of using the `which` crate](docs/decisions/0001-path-search.md)
- [ADR 0002: the command line](docs/decisions/0002-command-line.md)

## License

MIT.
