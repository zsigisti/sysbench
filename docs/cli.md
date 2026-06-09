# `crux` — CLI guide

CRUCIBLE's command-line front-end is a single binary, `crux`, plus a `sysinfo`
alias. This is the complete reference for the CLI application; the desktop GUI is
documented separately in [`gui/README.md`](../gui/README.md).

```
crux [GLOBAL FLAGS] [COMMAND]
```

If no command is given, `crux` runs `bench all` (the full suite).

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

```sh
crux                       # same as `crux bench all`
crux bench cpu             # CPU only
crux bench disk --dir /mnt/nvme   # storage test on a specific mount
```

After a run, `crux` (unless `--no-upload`):

1. **records** the run to local history (`~/.local/share/crucible/history`), and
2. **shares** it — to the CRUCIBLE score server by default, falling back to
   paste.rs if the server is unreachable — and prints the URL.

### `crux submit`

Run the full suite and submit it **to the CRUCIBLE score server only** (no
paste.rs fallback). Equivalent to `crux` but forces the server target, so the
machine shows up on the leaderboard at the configured server.

### `crux info`

Print the deep system report (see [sysinfo.md](sysinfo.md)). No benchmarking,
no recording, no upload.

### `crux compare <A> <B>`

Diff two runs. Each argument is a saved JSON file **or** a history id (see
`crux history`). Prints a per-metric table with % change; larger-is-better and
lower-is-better metrics are coloured accordingly on a TTY.

```sh
crux compare run-a.json run-b.json
crux compare 1700000000-amd-ryzen 1700100000-intel-core
```

### `crux history [list | show <id>]`

`crux history` (or `crux history list`) lists locally recorded runs with their
id, timestamp, CPU and composite scores. `crux history show <id>` prints the
stored JSON for one run (pipe it to a file to share or re-compare).

### `crux uninstall [--purge-data]`

Remove everything `install.sh` placed: the `crux` / `crux-gui` binaries, the
`sysinfo` alias, the man page, shell completions, and the desktop entry — from
both the per-user (`~/.local`, `~/.config`) and system (`/usr/local`, `/usr`)
locations it can write to. Local run history is **kept** unless `--purge-data`
is given. (System paths may need `sudo`.)

### `sysinfo`

If the binary is invoked under the name `sysinfo` (the installer creates this
symlink), it behaves exactly like `crux info`.

### `crux man` / `crux completions <shell>` (hidden)

Packaging helpers, hidden from `--help`. `crux man` prints a roff man page;
`crux completions <bash|zsh|fish|elvish|powershell>` prints a completion script.
Both write to stdout; the installer and packagers use them.

## Global flags

| Flag | Default | Applies to | Description |
|------|---------|-----------|-------------|
| `--json` | off | bench | Emit machine-readable JSON instead of the human report. A `# CRUCIBLE …` header precedes it. |
| `--output <FILE>` | — | bench | Also write the report (or `--json`) to `FILE`. |
| `--duration <secs>` | `10` | cpu | Seconds of measurement per CPU test. |
| `--runs <n>` | `5` | cpu | Measured runs per CPU test; one warmup run is discarded. Reports median ± stddev. |
| `--streams <n>` | `4` | net | Parallel HTTP streams for the download test. |
| `--dir <path>` | current dir | disk | Scratch-file directory; point at the real disk to measure (see tmpfs note). |
| `--no-upload` | off | bench | Skip sharing. |
| `--no-history` | off | bench | Skip recording to local history. |
| `--version` / `-V` | — | — | Print version. |
| `--help` / `-h` | — | — | Print help (works per subcommand too). |

## Sharing & the score server

- The default share target is the CRUCIBLE score server, base URL
  **`https://crux.mmzsigmond.me`**. Override it with the `CRUX_SERVER`
  environment variable (e.g. to point at your own deployment).
- `crux bench` uses the server with a **paste.rs fallback**; `crux submit`
  uses the server only.
- To host the server yourself, hand [`web.md`](../web.md) to a coding agent —
  it is a complete build runbook (Rust + axum + SQLite behind nginx). Fetch it
  with `scripts/get-server-guide.sh` or the one-liner inside it.

## Notes

- **tmpfs / `--dir`.** The storage test defaults to the CWD because `/tmp` is
  `tmpfs` (RAM) on most systemd distros — measuring it gives memory speed. `crux`
  detects a RAM-backed scratch dir and warns; use `--dir` for real storage.
- **Exit code.** A failed individual suite is reported inline and does not abort
  the others; `crux bench` still exits `0`.
- **JSON shape.** Mirrors the internal result structs: `sysinfo`, and
  `cpu` / `mem` / `net` / `disk` (null when a suite wasn't run). `disk` is either
  the results or `{ "error": "…" }`. `net.*` metrics are `Result`-encoded
  (`{"Ok": …}` / `{"Err": …}`).
- **History location.** `~/.local/share/crucible/history` (honours
  `XDG_DATA_HOME`).
