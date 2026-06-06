# sysbench

A system benchmark and information toolkit written in Rust. Ships two binaries:

- **`sysbench`** — benchmarks CPU, memory, network, and storage, prints a report, and uploads the results so you can share them.
- **`sysinfo`** — a thorough system information display (think `fastfetch`, but deeper).

---

## Install

```sh
curl -sSf https://raw.githubusercontent.com/zsigisti/sysbench/refs/heads/main/install.sh | bash
```

Or, if you already have the repo cloned:

```sh
bash install.sh
```

The script will:

- Install a C toolchain and Rust (via `rustup`) if they are missing
- Build with native CPU optimisations (LTO, single codegen unit, `target-cpu=native`)
- Install **both** `sysbench` and `sysinfo` to `~/.local/bin` (non-root) or `/usr/local/bin` (root)

---

## `sysbench`

```sh
sysbench                 # run everything (CPU, memory, network, storage)
sysbench cpu             # CPU suite only
sysbench mem             # memory bandwidth only
sysbench net             # network only
sysbench disk            # storage only
```

By default the results are uploaded to [paste.rs](https://paste.rs) and a share URL is printed at the end.

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--json` | off | Emit machine-readable JSON instead of the human report |
| `--duration <secs>` | `10` | Seconds per CPU test |
| `--runs <n>` | `5` | Measured runs per CPU test (plus one warmup) |
| `--streams <n>` | `4` | Parallel download streams for the network test |
| `--no-upload` | off | Do **not** upload results (upload is on by default) |

### What it measures

**CPU** — five workloads, each run single-threaded and multi-threaded with warmup and median ± stddev:

- **BBP-π** — hex digits of π via the Bailey–Borwein–Plouffe formula (integer-heavy)
- **SHA-256** — hashing 1 MiB blocks (MB/s)
- **MatMul** — 256×256 `f64` matrix multiply (GFLOPS)
- **LZ4** — compressing a 1 MiB semi-compressible buffer (MB/s)
- **Sort** — `sort_unstable` over 1M `u64` (M items/s)

A geometric-mean **composite score** and the MT/ST **speedup** are reported. Threads are pinned to distinct cores.

**Memory** — STREAM-style Copy / Scale / Add / Triad over 256 MiB arrays, GB/s. `black_box` barriers prevent the optimiser from deleting the kernels.

**Network** — Cloudflare speed-test endpoints: latency (min/avg/max/stddev/jitter), parallel download, and upload.

**Storage** — sequential write + read and random 4K read/write latency (p50/p99). On Linux it uses `O_DIRECT` to bypass the page cache. The test file is sized to fit available free space and is skipped cleanly if the disk is too full.

---

## `sysinfo`

```sh
sysinfo
```

Prints a single thorough report covering:

- OS, host, kernel + architecture, uptime, package counts, shell
- CPU model, physical/logical core counts, current/max frequency, full cache hierarchy
- Load average
- GPU(s) (via `lspci`, falling back to the DRM driver)
- Memory and swap usage with bars
- Every real mounted filesystem with usage bars
- Thermal sensors and battery state
- Network interfaces with MAC and IPv4/IPv6 addresses

Honours `NO_COLOR`.

---

## Build manually

```sh
cd tester
RUSTFLAGS="-C target-cpu=native" cargo build --release
./target/release/sysbench
./target/release/sysinfo
```

---

## License

GPL-3.0
