# JSON output

`dv --json NAME...` prints one JSON document on stdout.
The shape below is a stable contract: fields are only added, never removed or repurposed, without a bump of `version`.
Consumers should ignore fields they do not know.

## Shape

```json
{
  "version": 1,
  "results": [
    {
      "name": "rg",
      "found": true,
      "matches": [
        {
          "path": "/opt/homebrew/bin/rg",
          "path_index": 0,
          "winner": true,
          "duplicate_of": null,
          "symlink_chain": [
            {
              "path": "/opt/homebrew/bin/rg",
              "target": "../Cellar/ripgrep/14.1.1/bin/rg",
              "resolved": "/opt/homebrew/Cellar/ripgrep/14.1.1/bin/rg",
              "broken": false
            }
          ],
          "real_path": "/opt/homebrew/Cellar/ripgrep/14.1.1/bin/rg",
          "provenance": {
            "source": "homebrew",
            "package": "ripgrep",
            "version": "14.1.1",
            "detail": null
          }
        }
      ],
      "skipped": [
        { "path": "/usr/local/bin/rg", "path_index": 3, "reason": "not_executable" }
      ]
    }
  ]
}
```

Every key is always present; absent values are `null` or an empty array.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `version` | integer | Schema version. Currently `1`. |
| `results` | array | One entry per `NAME` given, in the order given. |
| `results[].name` | string | The name as given. |
| `results[].found` | boolean | Whether at least one runnable match exists. |
| `results[].matches` | array | Runnable matches in PATH order. The first is the one that runs. Empty if not found. |
| `matches[].path` | string | Where the match was found: the PATH entry joined with the name, made absolute against the current directory. Not canonicalized. |
| `matches[].path_index` | integer or null | Zero-based position of the PATH entry. `null` when the name contained a `/` and PATH was not searched. |
| `matches[].winner` | boolean | `true` for the first match only. |
| `matches[].duplicate_of` | integer or null | Index into `matches` of an earlier match that is the same file on disk (same device and inode), such as the same directory listed twice or reached through a symlinked directory. |
| `matches[].symlink_chain` | array | The symlinks followed from `path`, one per hop, in order. Empty if `path` is not a symlink. |
| `symlink_chain[].path` | string | The symlink. |
| `symlink_chain[].target` | string | The link's stored target, exactly as `readlink` returns it (possibly relative). |
| `symlink_chain[].resolved` | string | The absolute path the link points at, with directories in the way resolved. |
| `symlink_chain[].broken` | boolean | Nothing exists at `resolved`. Always `false` in `matches`, because a broken link is not runnable; see `skipped`. |
| `matches[].real_path` | string or null | The fully resolved file, with symlinked directories resolved too. `null` if it could not be resolved. |
| `matches[].provenance` | object or null | The package that owns the real file, or `null` if no detector recognised it. |
| `provenance.source` | string | `homebrew`, `cargo` or `nix`. More may be added. |
| `provenance.package` | string | Package, formula or crate name. |
| `provenance.version` | string or null | Version, when the source records one. |
| `provenance.detail` | string or null | Source-specific extra: for `cargo`, the git or path source when the crate was not installed from crates.io. |
| `results[].skipped` | array | Candidates that answer to the name but cannot run. |
| `skipped[].path` | string | The candidate. |
| `skipped[].path_index` | integer or null | As for `matches[].path_index`. |
| `skipped[].reason` | string | `not_executable`, `not_a_file` or `broken_symlink`. More may be added. |

## Behaviours worth knowing

- With `-1` / `--first`, `matches` holds only the winner, and `skipped` holds only the candidates met before it.
- Paths that are not valid UTF-8 are emitted with invalid bytes replaced by U+FFFD.
- The exit status is the same as without `--json`: 0 if every name was found, 1 if any was not, 2 on a usage error.
- `dv` writes diagnostics (such as `not found in PATH`) to stderr, never into the JSON.
