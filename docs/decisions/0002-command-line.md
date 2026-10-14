# 2. The command line: clap, flags only

Status: accepted.

## Context

`dv NAME...` looks up programs by name, and later versions will grow other modes: a shell hook (`init`), a PATH audit (`doctor`), and so on.
The command line has to be quick to start and must not get in the way of looking up a program with an unlucky name.

## Decision 1: use clap (derive)

The brief said to use clap unless it costs more than about 0.1 ms of startup over `lexopt`.
It does not.
The experiment is `experiments/argparse-startup`, with the numbers in `experiments/results/README.md`.

| Platform | bare `std::env::args` | `lexopt` | `clap` (derive, default features) |
| --- | ---: | ---: | ---: |
| macOS aarch64, dynamic | 1.52 ms | 1.44 ms | 1.44 ms |
| Linux aarch64, musl static | 0.10 ms | 0.10 ms | 0.12 ms |

- On macOS the three are indistinguishable.
  Clap executes about 0.3 M more instructions out of 13 M.
- On Linux static, where everything is cheapest, clap adds 0.02 to 0.03 ms, a quarter of the allowed budget.
- The price is binary size: about 200 KB more on macOS and 200 KB more on musl static.

Clap gives `--help`, `--version`, typo suggestions and consistent exit code 2 on usage errors for free.

## Decision 2: no subcommands, only flags

Every mode is a flag: `--json` today, `--init <shell>` and `--doctor` later.

A subcommand would collide with the thing the tool exists for.
If `dv` had an `init` subcommand, `dv init` would be ambiguous between "run the shell hook" and "where is the program called `init`?", and the same goes for `doctor`, `help` and anything added later.
Flags cannot collide this way, because a command name that begins with `--` is not a realistic problem, and `dv -- --odd-name` still works.

The test `a_program_named_init_is_just_a_name` keeps this true.

## Decision 3: the environment variable

`DV_EXPAND_TILDE` sets `--expand-tilde`, so that a shell hook can turn it on without wrapping the command.
It accepts the usual spellings (`1`, `true`, `yes`, `on` and their opposites), using clap's boolish parser.
Clap's default would have accepted only `true` and `false`, which would have surprised anyone writing `DV_EXPAND_TILDE=1`.
