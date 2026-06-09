# Architecture

CRUCIBLE is a Cargo **workspace**: a shared library crate (`crucible`) holds the
benchmark/report engine, the CLI (`crux`) and the optional Qt GUI (`crux-gui`)
are thin front-ends over it. A bare `cargo build` builds only the CLI
(`default-members`); the GUI is built explicitly with `-p crucible-gui` and is
the only part that needs Qt.

## Layout

```
.
├── Cargo.toml              # workspace root + `crucible` lib + `crux` bin + deb/rpm metadata
├── Cargo.lock              # committed for reproducible packaging
├── install.sh              # build-on-host installer (CLI + `sysinfo` symlink)
├── src/                    # the shared `crucible` library + `crux` CLI
│   ├── lib.rs              # library root: re-exports modules + run_suite_collect()/run_named() (used by the GUI)
│   ├── main.rs             # `crux` CLI (clap): bench / info / submit / compare / history / uninstall
│   ├── uninstall.rs        # remove installer artifacts (shared by CLI + GUI)
│   ├── affinity.rs         # CPU pinning that does NOT leak into MT workers
│   ├── stats.rs            # mean / median / stddev / percentile / geomean
│   ├── summary.rs          # flat digest of a results JSON (history / compare / leaderboard)
│   ├── history.rs          # local run history (record / list / load) under ~/.local/share/crucible
│   ├── compare.rs          # diff two runs → per-metric deltas + table
│   ├── sysinfo.rs          # lightweight SysInfo collector (bench result header)
│   ├── upload.rs           # share results: CRUCIBLE server (submit) + paste.rs fallback
│   ├── bench/
│   │   ├── mod.rs          # orchestration: Config, Suite, FullResults, format_results
│   │   ├── cpu.rs          # BBP / SHA-256 / MatMul / LZ4 / Sort (ST + MT)
│   │   ├── mem.rs          # STREAM Copy / Scale / Add / Triad
│   │   ├── net.rs          # Cloudflare latency / download / upload
│   │   └── disk.rs         # O_DIRECT seq + random I/O, tmpfs detection
│   └── report/
│       ├── mod.rs          # `crux info` renderer → render(color) -> String
│       ├── collect.rs      # /proc, /sys, getifaddrs collectors
│       └── format.rs       # Style/colour, human_bytes, bars, IPv6 compression
├── gui/                    # the optional Qt 6 GUI crate (`crux-gui`)
│   ├── Cargo.toml          # depends on `crucible` + cxx-qt
│   ├── build.rs            # cxx-qt-build: QML module + Rust bridge
│   ├── src/controller.rs   # `#[cxx_qt::bridge]` Controller QObject (threaded runs, history, compare, export, uninstall)
│   ├── src/main.rs         # QGuiApplication + QQmlApplicationEngine
│   └── qml/                # main.qml (tabbed UI) + Theme.qml + Card.qml
├── packaging/
│   ├── aur/PKGBUILD        # Arch — builds from source on host
│   ├── deb/{postinst,prerm}# manage the `sysinfo` alias on deb installs
│   ├── gen-assets.sh       # generate man page + completions for deb/rpm
│   └── crux-gui.desktop    # desktop entry for the GUI (used when packaging)
├── scripts/
│   └── get-server-guide.sh # one-liner to fetch web.md (score-server runbook)
├── web.md                  # AI-agent runbook to build the score server
└── docs/                   # this documentation
```

Both front-ends call the same engine, so CLI and GUI always report identical
numbers. The CLI prints via `bench::format_results` / `report::render`; the GUI
shows the very same strings (with colour off) in its results pane.

## Module responsibilities

- **`main.rs`** — argument parsing (clap derive), the `sysinfo`-alias
  short-circuit, suite dispatch, JSON vs human output, `--output`, the
  record-to-history and share/submit steps, and the `compare` / `history` /
  `uninstall` subcommands.
- **`affinity.rs`** — the single source of truth for CPU pinning. `PinGuard`
  pins for an ST run and restores all cores on drop; `reset_to_all_cores` is
  called before spawning MT workers. See [methodology.md](methodology.md) for
  why this matters.
- **`bench/mod.rs`** — owns `Config` (tunables), `Suite` (selection),
  `FullResults` (serialisable output), and all human-readable printers. It is
  the only place that knows how to run a suite and how to display it.
- **`bench/*.rs`** — one file per suite; each exposes a `run(...)` returning a
  `Serialize` results struct. No suite prints its own final report (the printers
  live in `mod.rs`); they only emit per-run progress lines.
- **`report/`** — completely independent of `bench`. `collect.rs` returns plain
  data; `format.rs` is pure formatting; `mod.rs` glues them into the table.
- **`sysinfo.rs` vs `report/`** — intentionally separate. `sysinfo.rs` is a tiny
  serialisable struct embedded in benchmark JSON (cpu model, cores, RAM, kernel,
  OS). `report/` is the rich human-only `crux info` view.

## Data flow

```
crux bench all
  └─ sysinfo::SysInfo::collect()        # header / JSON metadata
  └─ bench::run(Suite::All, &cfg, info) # runs each suite
       ├─ cpu::run    → CpuResults
       ├─ mem::run    → MemResults
       ├─ net::run    → NetResults
       └─ disk::run   → DiskResults | DiskOutcome::Err
  └─ bench::print_results(&full, &cfg)  # human output
  └─ history::record(json)              # unless --no-history
  └─ upload::share(json)                # unless --no-upload (server → paste.rs fallback)
```

The score server (`web.md`) accepts the same `FullResults` JSON, extracts the
`summary.rs` digest into SQLite columns, and ranks machines on a leaderboard.

## Dependencies

Deliberately minimal: `clap` (CLI), `serde`/`serde_json` (output), `ureq`
(HTTP for net + upload), `sha2`, `lz4_flex`, `core_affinity`, and `libc` on
Unix. Everything else (sysinfo, STREAM, O_DIRECT, getifaddrs) is hand-rolled
against `/proc`, `/sys`, and libc.
