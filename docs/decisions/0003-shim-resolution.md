# 3. Shim resolution for mise, asdf and pyenv

Status: proposed, awaiting review.

## Context

Version managers put a directory of _shims_ at the front of PATH.
A shim is a stand-in for every command any installed version provides, and it decides at run time which real file to execute.
So today `dv python` on a machine that uses pyenv answers "a tiny script in `~/.pyenv/shims`", which is true but is not the answer to "where did this command come from?".
The answer the user wants is "Python 3.13.7, installed by pyenv, selected by `~/.pyenv/version`".

The original brief fixed the shape: the default mode reads the manager's files (fast), and an opt-in mode runs the manager's own `which` (accurate, slower).
This record works out what each mode can honestly do for each manager, and what to build first.

### Evidence and its limits

- The pyenv findings are checked against source files and against a real `~/.pyenv` on this machine.
- The mise and asdf findings come from their official documentation and source, read by a research pass.
  Neither manager is installed here, so nothing about them has been run.
- Facts that could not be confirmed are marked **unverified** and are listed again under "Verify before building".

## What a shim is, per manager

| | pyenv | asdf | mise |
| --- | --- | --- | --- |
| Physical form | A ~181-byte bash script, identical for every command except the name it takes from `$0`. | A bash script with `# asdf-plugin: <plugin> <version>` comment lines, then `exec asdf exec "<cmd>" "$@"`. | On Unix, a symlink to the `mise` binary. Plugin-style shims are generated shell scripts ending in `mise x -- …`. |
| Shims directory | `$PYENV_ROOT/shims`, default `~/.pyenv/shims` | `$ASDF_DATA_DIR/shims`, default `~/.asdf/shims` | `<data dir>/shims`, default `~/.local/share/mise/shims`; `MISE_SHIMS_DIR` overrides it |
| What the shim records | The pyenv binary path and `PYENV_ROOT`, hardcoded. | Which plugins provide the command (comment lines). | Nothing but the command name. |
| Real binaries | `$PYENV_ROOT/versions/<v>/bin/<cmd>` | `$ASDF_DATA_DIR/installs/<plugin>/<v>/bin/<cmd>`, but plugins can change the subdirectory | `<data>/installs/<tool>/<v>/…`, with the subpath chosen by the backend |

The local pyenv shim shows what a stale shim looks like.
It hardcodes `PYENV_ROOT="/Users/nick/.pyenv"`, but this machine's real directory is `/Users/nickv/.pyenv`.
A resolver that trusts the shim's own `PYENV_ROOT` would look in the wrong place, so the real root must come from the environment first and the shim second.

## How each manager picks a version

All three read, in order: an environment variable, then a version file found by walking up from the working directory, then a global default.

| | pyenv | asdf | mise |
| --- | --- | --- | --- |
| Environment | `PYENV_VERSION` (colon-separated list) | `ASDF_<TOOL>_VERSION` | `MISE_<TOOL>_VERSION` |
| Per-directory files | `.python-version` (walking up) | `.tool-versions` (walking up, then `$HOME`) | `mise.local.toml`, `mise.toml`, `.mise.toml`, `.config/mise*`, `.tool-versions`, merged from the root down to the cwd |
| Global | `$PYENV_ROOT/version` | `$HOME/.tool-versions` | `~/.config/mise` and `/etc/mise` |
| Legacy and idiomatic files | `.pyenv-version`, only without `.python-version` in the same directory | `legacy_version_file` in `~/.asdfrc`, off by default, with filenames supplied by the plugin | `.nvmrc`, `.python-version` and similar, only for tools enabled in settings (default **unverified**) |
| Special values | `system`, partial versions such as `3.12`, a `python-` prefix | `system`, `path:`, `ref:`, several versions on one line | `latest`, `lts`, prefixes, `path:`, `ref:`, `system`, version lists |

## Which of this can be done by reading files

The honest answer is different for each manager.

**pyenv: almost exactly.**
The whole algorithm is file reads and stats: pick the version list, then for each version check `versions/<v>/bin/<cmd>` for an executable, and treat `system` as PATH with the shims directory removed.
Gaps: plugin hooks (pyenv-virtualenv and others) can change the answer and cannot be seen, and a partial version like `3.12` needs "latest installed match" logic.

**asdf: mostly.**
`.tool-versions` is a simple format, and the shim file itself names the plugins.
Gaps: `list-bin-paths` and `exec-path` are plugin scripts that decide the real subdirectory and executable name, and they cannot be read without running them.
The default `bin/` subdirectory is right for most plugins but not all.
When several plugins provide a command, which one wins is **unverified** (the research flagged non-deterministic ordering in the Go rewrite).
The Go rewrite (0.16 and later) changed the shim to call `asdf` from PATH rather than by absolute path, so the two generations must both be recognised.

**mise: only approximately.**
Config is TOML with templates and a trust mechanism, and untrusted files are skipped or error out; a simple reader cannot evaluate either.
The shim is a bare symlink, so the command name does not say which tool provides it (`npm` comes from `node`), and the subpath below the install directory depends on the backend.
mise's own documentation says to use `mise which` rather than building these paths by hand.

## Decision

### 1. Detect shims by directory name and content

A match is checked as a possible shim only if its directory is named `shims`, or it is a symlink whose target is a file called `mise`.
Only then is the shim read (the first 1 KB), and it is recognised by its content: `pyenv exec`, `# asdf-plugin:` lines with `asdf exec`, or a symlink to `mise`.
Ordinary lookups pay nothing, and relocated shims directories are found without any configuration.

### 2. Two modes, selected with `--shims <MODE>`

| Mode | What it does | Default |
| --- | --- | --- |
| `files` | Reads version files and the install tree. Never runs anything. | yes |
| `run` | Asks the manager (`pyenv which`, `asdf which`, `mise which`). Exact, slower. | no |
| `off` | Reports the shim as an ordinary file. | no |

`DV_SHIMS` sets the mode from the environment, so a shell hook can choose it.
`--exact` is not added as a second spelling, to keep one way to do it.

### 3. Say how sure we are

Every resolved shim carries a confidence: `exact` (the manager answered), `derived` (read from files, no known gap) or `approximate` (read from files, with a named gap such as "plugin hooks not consulted" or "config trust not evaluated").
Where the files cannot answer, the result is `unresolved` with a reason, and the shim is still shown.
`dv` never guesses silently.

### 4. Run mode is the one exception to "never execute what we inspect"

The brief says `dv` never executes the binaries it inspects.
Run mode breaks that rule on purpose, so it is opt-in, and it is limited:

- It runs only the manager's own documented, read-only `which` subcommand, never the program the user asked about, and never with the user's arguments.
- It runs with a timeout (2 seconds to start with) and captures output, so a hanging plugin cannot hang `dv`.
- It runs in the current directory with the current environment, because the answer depends on both.
- Be aware that `pyenv which` sources hook scripts from the pyenv plugins directory and `asdf which` can run plugin scripts, so it executes code the user has installed.
  That is the same exposure as running the shimmed command, but it is code `dv` triggers, so the documentation must say so.
- Whether `mise which` ever auto-installs a missing tool is **unverified**, and must be settled (and disabled by environment variable if possible) before this mode ships.

### 5. Shape of the output

The version manager becomes a provenance source for the resolved file (`pyenv`, `asdf`, `mise`), using the install path (`versions/<v>/`, `installs/<plugin>/<v>/`, `installs/<tool>/<v>/`) the same way the Homebrew and Nix detectors use theirs.

JSON gains one new field on each match, `shim`, which is `null` for ordinary files:

```json
"shim": {
  "manager": "pyenv",
  "mode": "files",
  "confidence": "derived",
  "resolved": "/Users/me/.pyenv/versions/3.13.7/bin/python",
  "selected_by": { "kind": "file", "path": "/Users/me/.pyenv/version", "value": "3.13.7" },
  "caveats": []
}
```

This is an addition, so the JSON `version` stays at 1.
Text output gains lines under the shim match:

```text
* /Users/me/.pyenv/shims/python
    shim: pyenv, 3.13.7 (from /Users/me/.pyenv/version)
    -> /Users/me/.pyenv/versions/3.13.7/bin/python3.13
    pyenv: python 3.13.7
```

### 6. Structure

One trait, one implementation per manager, sitting next to the provenance detectors:

```text
trait ShimResolver {
    fn recognise(&self, shim: &MatchedFile) -> Option<Shim>;
    fn resolve_files(&self, shim: &Shim, ctx: &Context) -> Resolution;
    fn resolve_run(&self, shim: &Shim, ctx: &Context) -> Resolution;
}
```

`Context` holds the working directory and environment, passed in so tests can fake both.
The pyenv resolver is written so that rbenv and nodenv, which share its design, can reuse it (a claim from memory, **unverified**).

## Order of work

1. **Recognition and the output shape**, with `run` mode for pyenv, whose `pyenv which` is safe to run and testable here against the real `~/.pyenv`.
2. **pyenv `files` mode**, the best-understood algorithm, checked against `pyenv which` on this machine.
3. **asdf `files` mode and asdf `run`.**
4. **mise `run` mode.**
5. **mise `files` mode last, and only if it proves worth the approximations.**
   Until then, `--shims files` on a mise shim reports `unresolved` with the reason, and points at `--shims run`.

Each step is a separate PR.

## Testing

- Each resolver gets a miniature fake tree, as the provenance detectors have (a fake `~/.pyenv` with shims, versions and a version file), so the tests pass on any machine.
- The pyenv fake tree can be checked against real pyenv, because pyenv is a bash script that CI can install from git.
  That differential test, comparing `files` against `run` on the same tree, is the best protection against the fast mode drifting from the manager, and I would add it for asdf and mise as they land.
- Neither manager is installed here, so asdf and mise `files` mode would be written against documentation alone until that CI job exists.

## Verify before building

1. Does `mise which` auto-install, what are its exit codes, and is there an environment variable to stop it?
2. Which idiomatic version files does mise read by default?
3. How does Go asdf choose between plugins that provide the same command?
4. What `asdf shimversions` prints, and whether it is a better source than parsing the shim.
5. The multi-version file format for pyenv (whitespace or newline separated).

## Open questions

1. Is `--shims files|run|off` the right shape, or would you rather have a boolean `--exact`?
2. Is a 2-second timeout right for run mode?
3. Should the mise `files` mode exist at all, or is `run` mode enough for mise?
4. Is it acceptable that run mode executes manager hooks and plugin scripts, given that it is opt-in and documented?
