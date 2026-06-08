# CLI Reference

CRUCIBLE ships a single binary, `crux`, with two subcommands plus a `sysinfo`
alias.

```
crux [GLOBAL FLAGS] [COMMAND]
```

If no command is given, `crux` runs `bench all`.

## Commands

### `crux bench [SUITE]`

Run benchmarks. `SUITE` is one of:

| Suite | Runs |
|-------|------|
| `all` *(default)* | CPU, memory, network, storage |
| `cpu` | CPU suite only |
| `mem` | Memory bandwidth only |
| `net` | Network only |
| `disk` | Storage only |

Examples:

```sh
crux                       # same as `crux bench all`
crux bench cpu             # CPU only
crux bench disk --dir /mnt/nvme   # storage test on a specific mount
```

At the end, results are uploaded to paste.rs and a share URL is printed unless
`--no-upload` is passed.

### `crux info`

Print the deep system report (see [sysinfo.md](sysinfo.md)). No benchmarking and
no upload ever happen in this mode.

### `sysinfo`

If the binary is invoked under the name `sysinfo` (the installer and packages
create this symlink), it behaves exactly like `crux info`.

### `crux man` / `crux completions <shell>` (hidden)

Packaging helpers, hidden from `--help`. `crux man` prints a roff man page;
`crux completions <bash|zsh|fish|elvish|powershell>` prints a completion script.
Both write to stdout. The installer and packagers use these to install
`/usr/share/man/man1/crux.1` and shell completions.

## Global flags

| Flag | Default | Applies to | Description |
|------|---------|-----------|-------------|
| `--json` | off | bench | Emit machine-readable JSON instead of the human report. A `# CRUCIBLE …` comment header with host summary precedes the JSON. |
| `--duration <secs>` | `10` | cpu | Seconds of measurement per CPU test. |
| `--runs <n>` | `5` | cpu | Measured runs per CPU test; one extra warmup run is always discarded. The median ± stddev is reported. |
| `--streams <n>` | `4` | net | Parallel HTTP streams for the download test. |
| `--dir <path>` | current dir | disk | Directory to place the scratch file in. Pick a path on the **real disk** you want to measure — see the tmpfs note below. |
| `--no-upload` | off | bench | Skip uploading results. |
| `--version` / `-V` | — | — | Print version. |
| `--help` / `-h` | — | — | Print help (works per subcommand too). |

## Notes

- **tmpfs / `--dir`.** The storage test defaults to the current working
  directory because `/tmp` is `tmpfs` (RAM) on most modern systemd distros —
  measuring it gives memory speed, not disk. `crux` detects a RAM-backed scratch
  directory and prints a loud warning; use `--dir` to point at real storage.
- **Exit code.** A failed individual suite (e.g. no free disk space, no network)
  is reported inline and does not abort the others; `crux` still exits `0`.
- **JSON shape.** The JSON mirrors the internal result structs: `sysinfo`, and
  `cpu` / `mem` / `net` / `disk` objects (null when a suite wasn't run). The
  `disk` object is either the results or `{ "error": "…" }`.
