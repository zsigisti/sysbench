# Architecture

CRUCIBLE is a single Rust crate (`crucible`) producing one binary (`crux`). The
codebase is organised by responsibility, with the benchmark suites and the
system-report renderer as separate module trees.

## Layout

```
.
├── Cargo.toml              # crate `crucible`, bin `crux`, deb/rpm metadata
├── Cargo.lock              # committed for reproducible packaging
├── install.sh              # build-on-host installer (+ `sysinfo` symlink)
├── src/
│   ├── main.rs             # `crux` CLI (clap): dispatch bench / info + upload
│   ├── affinity.rs         # CPU pinning that does NOT leak into MT workers
│   ├── stats.rs            # mean / median / stddev / percentile / geomean
│   ├── sysinfo.rs          # lightweight SysInfo collector (bench result header)
│   ├── upload.rs           # POST results to paste.rs
│   ├── bench/
│   │   ├── mod.rs          # orchestration: Config, Suite, FullResults, printers
│   │   ├── cpu.rs          # BBP / SHA-256 / MatMul / LZ4 / Sort (ST + MT)
│   │   ├── mem.rs          # STREAM Copy / Scale / Add / Triad
│   │   ├── net.rs          # Cloudflare latency / download / upload
│   │   └── disk.rs         # O_DIRECT seq + random I/O, tmpfs detection
│   └── report/
│       ├── mod.rs          # `crux info` renderer (assembles + prints rows)
│       ├── collect.rs      # /proc, /sys, getifaddrs collectors
│       └── format.rs       # Style/colour, human_bytes, bars, IPv6 compression
├── packaging/
│   ├── aur/PKGBUILD        # Arch — builds from source on host
│   └── deb/{postinst,prerm}# manage the `sysinfo` alias on deb installs
└── docs/                   # this documentation
```

## Module responsibilities

- **`main.rs`** — argument parsing (clap derive), the `sysinfo`-alias
  short-circuit, suite dispatch, JSON vs human output, and the upload step.
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
  └─ upload::upload(json)               # unless --no-upload
```

## Dependencies

Deliberately minimal: `clap` (CLI), `serde`/`serde_json` (output), `ureq`
(HTTP for net + upload), `sha2`, `lz4_flex`, `core_affinity`, and `libc` on
Unix. Everything else (sysinfo, STREAM, O_DIRECT, getifaddrs) is hand-rolled
against `/proc`, `/sys`, and libc.
