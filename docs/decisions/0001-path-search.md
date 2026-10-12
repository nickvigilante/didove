# 1. Walk PATH ourselves instead of using the `which` crate

Status: proposed, awaiting review.

## Context

`dv` answers "where did this command come from?", which starts with "which files on PATH match this name, in what order, and which one runs?".
The `which` crate (harryfei/which-rs, v8.0.6) is the obvious dependency for the first half.
This record compares it with a small PATH walk of our own, using evidence from the crate's source, an edge-case test matrix, and benchmarks.

What `dv` needs that a plain `which` does not:

- Every match, in order, with the shadowed ones marked.
- The position of each PATH entry and why an entry was skipped, because the planned `dv --doctor` audits PATH.
- Exec semantics that match what the kernel will do when the shell runs the command.

Everything lives in `experiments/`: the crate `path-search` (both implementations, the tests and the benchmark binaries), `linux-bench.sh`, `summarize.py` and `results/README.md` with the full tables.

## What the `which` crate does (v8.0.6, read from source)

| Question | Answer | Where |
| --- | --- | --- |
| How does it decide a file is executable? | `metadata(path).is_file()` (follows symlinks), then `libc::access(path, X_OK)`. This is a real access check, not a mode-bit test. It uses the **real** UID/GID, whereas `execve` checks the **effective** one. The two differ only in setuid/setgid processes. | `checker.rs:63`, `sys.rs:212` |
| Empty PATH entries? | `std::env::split_paths` yields an empty path, which is joined with the name and made absolute against the cwd. So an empty entry means the cwd, as in POSIX. (Only Windows drops empty entries.) | `finder.rs:170-185` |
| Relative entries? | Made absolute against the cwd you pass (or the process cwd). | `finder.rs:176-183` |
| Trailing spaces? | Not trimmed. | no trimming code anywhere |
| Nonexistent directories? | Silently skipped. The failure goes to an optional handler as a bare `io::Error`. | `error.rs`, `checker.rs:63-68` |
| Duplicates? | Not deduplicated. The same file is returned once per occurrence. | `finder.rs` |
| Tilde? | A leading `~` component is **expanded to the home directory**. `execvp` does not (checked on macOS: `PATH='~/bin' env tool` exits 127; not checked on Linux). | `finder.rs:177`, `finder.rs:294` |
| Which PATH entry produced a match, at what position? | Not available. The iterator yields `PathBuf` and nothing else. | `finder.rs` (`Iterator::Item = PathBuf`) |
| Why was an entry skipped? | Only as an `io::Error` such as "Permission denied (os error 13)", with **no path and no position attached**. A directory in the way produces no error at all. | `error.rs`, `checker.rs` |
| Canonicalizes or follows symlinks in the result? | No. The returned path is the PATH entry joined with the name. Canonicalization exists only through the separate `CanonicalPath` wrapper, which fails outright when canonicalizing fails. | `lib.rs:617` |

Dependency tree (`cargo tree -e normal`): `which` adds one crate, `libc`, on Unix.
It is 1,707 lines including Windows and wasm support, plus optional `regex` and `tracing` features that we would not enable.

## Edge-case matrix

Each row is a tempdir fixture run through three implementations: the `which` crate (`WhichConfig` with a custom PATH and cwd), our walker (`walk` in `experiments/path-search/src/lib.rs`), and a deliberately naive control that treats any exec bit as runnable and trims PATH entries, which are the two `whence` bugs.
Run it with `cargo run --features scenarios --example matrix`.
The command is `tool`; `$T` is the fixture root.

| Case | `which` 8.0.6 | our walk | naive control |
| --- | --- | --- | --- |
| Non-executable file (0644) before an executable one | skips it, returns `b/tool` | same, records `NotExecutable` at position 0 | same |
| Mode 0011 file that we own, before an executable one | skips it | same, records `NotExecutable` | **wrong**: returns `a/tool` too |
| Directory named `tool` before an executable file | skips it, **no error reported** | same, records `NotAFile` | same |
| Symlink to a non-executable file | skips it | same, records `NotExecutable` | same |
| Broken symlink | skips it | same, records `BrokenSymlink` (via a second `lstat`) | same |
| Symlink to an executable | returns the symlink path, not the target | same | same |
| Empty PATH entry | `cwd/tool`, then `b/tool` | same, entry recorded as empty | same |
| `.` entry | `cwd/tool` | same | `cwd/./tool` (cosmetic) |
| Relative entry `rel/bin` | `cwd/rel/bin/tool` | same | same |
| Directory name ends in a space | **found** | found | **wrong**: missed |
| Same directory listed twice | two identical results | two results, the second flagged `same_file_as: 0` | two identical results |
| Same file via a symlinked directory (`bin` → `usr/bin`) | two results, no link between them | two results, the second flagged `same_file_as: 0` | two results |
| Nonexistent directory first | skips it, error without a path | same, records `Absent` | same |
| Root-owned 0700 file before an executable one | skips it | skips it, `NotExecutable` | **wrong**: returns `a/tool` too |

On the last row: it needs a fixture only root can build, so `linux-bench.sh` creates it in a container and runs the test as an unprivileged user (`file_executable_only_by_another_user`).
It passes on Linux, and on macOS the test reports itself skipped because there is no root.
The mode 0011 row is the same bug reproduced without root.

The `which` crate and our walker agree on every match list.
Both are correct on both `whence` bugs, so there is no correctness reason to leave `which`.
The control fails exactly where `whence` did, so the tests can tell a wrong implementation from a right one.

Not tested: real versus effective UID (needs a setuid process), and `~` in tests (checked separately, see below).

### Tilde, checked separately

`PATH='~/bin'`, `HOME` pointing at a directory containing `bin/tool`:

| Resolver | Finds `tool`? |
| --- | --- |
| `which` crate | yes |
| our walk, and `execvp` on macOS (via `env`) | no |
| bash (`command -v`) and macOS `/bin/sh` | yes |
| zsh (`command -v`) | no |

Shells disagree, so this is a policy choice rather than a bug.
Our walker does not expand `~`, because it reports what the kernel would run, and a literal `~` directory is what `execvp` sees.

## Benchmarks

`hyperfine --warmup 50 -N --runs 1000`, looking up `cargo` (found) and `nosuchcmd-xyz` (missing).
Mean wall-clock milliseconds; the full tables with standard deviation, medians and instruction counts are in `experiments/results/README.md`.

### macOS 27 (aarch64, release build, dynamically linked, 23 PATH entries)

| Command | `cargo` | missing |
| --- | ---: | ---: |
| our walk | 1.76 | 1.72 |
| `which` crate | 1.65 | 1.79 |
| `/usr/bin/which` | 0.97 | 1.04 |
| `sh -c 'command -v …'` | 2.88 | 2.93 |
| baselines: `/usr/bin/true` 0.88, `sh -c :` 2.65 | | |

Instruction counts per run: walk 14.2 M, `which` crate 13.8 M, `/usr/bin/which` 7.4 M.

### Linux (aarch64 Colima VM, `rust:1` container, 7 PATH entries)

| Command | `cargo` | missing |
| --- | ---: | ---: |
| walk, glibc dynamic | 0.23 | 0.25 |
| `which` crate, glibc dynamic | 0.21 | 0.23 |
| walk, glibc `+crt-static` | 0.14 | 0.15 |
| `which` crate, glibc `+crt-static` | 0.13 | 0.15 |
| walk, musl static | 0.10 | 0.13 |
| `which` crate, musl static | 0.10 | 0.10 |
| `/usr/bin/which` (Debian) | 0.23 | 0.26 |
| `busybox which` (static C) | 0.14 | 0.14 |
| `sh -c 'command -v …'` | 0.19 | 0.20 |
| baselines: `/bin/true` 0.17, `sh -c :` 0.19 | | |

### What the numbers say

- The `which` crate and our walk are indistinguishable in every build mode.
  Differences are within one standard deviation.
- Startup is almost the whole cost.
  On Linux, going from dynamic to static linking saves about 0.1 ms (0.23 → 0.10–0.14 ms), far more than any difference between the search implementations.
- A static musl Rust binary (0.10 ms) beats static C `busybox which` (0.14 ms) in this environment, and dynamic glibc is about 0.06 ms above the bare `/bin/true` floor.
- On macOS static linking is not available, so the Rust binaries sit about 0.8 ms above the `/usr/bin/true` floor and `/usr/bin/which` wins by about 0.7 ms.
  I did not investigate where the Rust binaries' extra 0.8 ms goes; they execute about twice the instructions (14 M against 7 M).
- `command -v` adds 0 to 0.2 ms on top of starting the shell (2.65 ms for `sh -c :` against 2.88 ms on macOS, 0.19 against 0.19 on Linux), so shell startup dominates it.

### Binary sizes (bytes, release profile: `lto`, `codegen-units = 1`, `panic = "abort"`, `strip`)

| Build | walk | `which` crate |
| --- | ---: | ---: |
| macOS aarch64 dynamic | 319,312 | 319,312 |
| Linux glibc dynamic | 332,504 | 332,504 |
| Linux glibc `+crt-static` | 994,080 | 1,125,152 |
| Linux musl static | 397,472 | 397,472 |

The identical sizes are real: the two binaries differ in content (the bytes differ), but `std` dominates and the difference is absorbed by section alignment.
The one visible gap is static glibc, where the walker is 131 KB smaller; I did not confirm the cause, but the likely one is that `which` calls glibc's `access` while rustix issues raw syscalls on Linux.

### Caveats

- The Linux numbers come from an aarch64 Colima VM, not bare metal, and the PATH there has 7 entries against 23 on the Mac.
  Absolute values are not comparable with your earlier x86 numbers (1.2 ms), and I could not reproduce those.
  The ordering is what carries over: both implementations tie, static beats dynamic, C `which` does not beat a static Rust binary.
- `/usr/bin/which` is a different program on each OS (Apple's binary on macOS, Debian's `debianutils` script on Linux), so its row is a rough reference only.
- No x86_64 numbers.

## Dependency weight

| Option | Crates added (Linux) | Crates added (macOS) |
| --- | --- | --- |
| `which` | `which`, `libc` | `which`, `libc` |
| own walk with `rustix` | `rustix`, `bitflags`, `linux-raw-sys` | `rustix`, `bitflags`, `errno`, `libc` |
| own walk with `libc` directly | `libc` | `libc` |

`rustix` is a safe API, so it avoids `unsafe` in our code, and it was already in the scaffold's `Cargo.toml`.
Calling `libc::faccessat` directly would match `which`'s dependency count at the price of one `unsafe` block.

## Decision

**Walk PATH ourselves.**
Use `rustix::fs::accessat(CWD, path, Access::EXEC_OK, AtFlags::EACCESS)` for the executable test, so it uses the effective UID like `execve`.

Reasons, in order of weight:

1. **`which` cannot give us what `dv` is for.**
   It returns only paths.
   `dv` needs the PATH position, the reason each entry was skipped, and duplicate detection.
   The only way to get these from `which` is to call it once per entry, and then we would run our own `stat` anyway to learn why it declined.
2. **There is no correctness or speed gain from staying.**
   The `which` crate passes every edge case, including the two that sank `whence`, and it ties on speed.
   The walker is about 60 lines and the matrix above covers it.
3. **Policy belongs in our code.**
   Tilde expansion, real versus effective UID and the treatment of directories are decisions we want to make explicitly, and test.
4. **Binary size and Windows are non-issues.**
   Windows support is out of scope, and size is within noise except for static glibc, where we are smaller.

What the walker records per entry (and what `--doctor` will reuse): index, the entry as written, the resolved candidate, an outcome (`Executable`, `NotExecutable`, `NotAFile`, `BrokenSymlink`, `Absent`, `Error(kind)`), and `same_file_as` for entries that resolve to a file already seen.

### Policies the walker adopts

- Empty entry and `.` mean the cwd.
  Relative entries resolve against the cwd and are reported as written.
- No trimming, no tilde expansion, no deduplication.
  Duplicates are listed but flagged, so `dv` can print them once and `--doctor` can warn.
- Symlinks are not resolved during the search.
  The search reports the path as found; hop-by-hop resolution is a separate step.
- `Absent` does not distinguish a missing directory from a missing file, to avoid a second `stat` on every miss.
  `--doctor` can `stat` the directories itself, once per entry rather than once per name.

### What would change this

- If we later wanted Windows support, `which` handles `PATHEXT` and `.exe` casing, which the walker does not.
- If you would rather have `libc` than `rustix`, say so and the walker moves over with one `unsafe` call.

## Open questions

1. Is not expanding `~` right?
   It matches zsh and macOS `execvp` but not bash.
2. Should `Absent` be split into missing-directory and missing-file (one extra `stat` per miss), or left to `--doctor`?
3. Is `rustix` (safe, three crates) preferred over `libc` (one `unsafe` call, one crate)?

## Reproducing

```bash
cd experiments/path-search
cargo test --features scenarios
cargo run --features scenarios --example matrix
cargo run --example tilde

cd ..
docker run --rm -v "$PWD:/exp:ro" -v "$PWD/results:/out" rust:1 sh /exp/linux-bench.sh
python3 summarize.py results/linux-aarch64.json
```
