<div align="center">

<img src="assets/logo.svg" alt="CRUCIBLE" width="160">

# CRUCIBLE

**Trial by fire for your machine.**

A host-native CPU / memory / network / storage benchmark **and** a deep
system-info tool — one binary, `crux`, written in Rust with no runtime
dependencies.

`crux bench` · `crux info`

</div>

---

CRUCIBLE compiles **natively on your machine** and puts it through a gauntlet:

- **`crux bench`** — multi-threaded CPU suites, STREAM memory bandwidth, a
  Cloudflare network test, and `O_DIRECT` storage I/O. Prints a report and
  (by default) uploads a shareable copy.
- **`crux info`** — a fast, thorough system report. Think `fastfetch`, but
  deeper: full cache hierarchy, every mounted disk, thermals, batteries, and
  per-interface IPv4/IPv6.
- **`crux-gui`** — an optional Qt 6 desktop GUI over the same engine. The CLI is
  fully standalone; the GUI lives in its own crate ([`gui/`](gui/README.md)) and
  is never required.

The name is an acronym for what it measures:
**C**ompute · **R**AM · **U**tilization · **C**ache · **I**/O · **B**andwidth ·
**L**atency **E**valuation.

---

## Install

### Quick (build-on-host script)

The CLI and GUI install **independently** — install either, or both:

```sh
# CLI only (crux + sysinfo alias) — the default
curl -sSf https://raw.githubusercontent.com/zsigisti/crucible/refs/heads/main/install.sh | bash

# GUI only (crux-gui + app-menu entry) — needs Qt 6
curl -sSf https://raw.githubusercontent.com/zsigisti/crucible/refs/heads/main/install.sh | bash -s -- --gui

# both
curl -sSf https://raw.githubusercontent.com/zsigisti/crucible/refs/heads/main/install.sh | bash -s -- --all
```

The script installs a C toolchain + Rust (and Qt 6 for `--gui`) if missing,
builds with `-C target-cpu=native`, and installs to `~/.local/bin` (or
`/usr/local/bin` as root). The CLI install also adds the `sysinfo` alias, the
`man crux` page, and shell completions; the GUI install also adds a desktop
entry + icon. From a local clone: `./install.sh [--gui|--all]`.

### Packages (AUR / deb / rpm)

CRUCIBLE is designed to be **built on the target host** so the benchmark
reflects that machine. See **[docs/packaging.md](docs/packaging.md)** for the
AUR `PKGBUILD`, `cargo deb`, and `cargo generate-rpm` workflows.

### From source

```sh
git clone https://github.com/zsigisti/crucible
cd crucible
RUSTFLAGS="-C target-cpu=native" cargo build --release
./target/release/crux            # full benchmark
./target/release/crux info       # system report
```

---

## Usage

```sh
crux                     # full benchmark (CPU + memory + network + storage)
crux bench cpu           # one suite: cpu | mem | net | disk | all
crux info                # deep system report (no benchmarking, no upload)
sysinfo                  # alias for `crux info`
```

By default `crux bench` uploads results to [paste.rs](https://paste.rs) and
prints a share URL. Disable with `--no-upload`.

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--json` | off | Machine-readable JSON instead of the human report |
| `--duration <secs>` | `10` | Seconds per CPU test |
| `--runs <n>` | `5` | Measured runs per CPU test (plus one warmup) |
| `--streams <n>` | `4` | Parallel download streams for the network test |
| `--dir <path>` | CWD | Scratch directory for the storage test |
| `--no-upload` | off | Do **not** upload results (upload is on by default) |

Full reference: **[docs/cli.md](docs/cli.md)**.

### GUI (optional)

A Qt 6 desktop front-end lives in [`gui/`](gui/README.md) as a separate crate;
the CLI never depends on it. Install it with `install.sh --gui` (above), or
build it directly:

```sh
cargo build -p crucible-gui --release   # needs Qt 6 (qt6-base, qt6-declarative)
./target/release/crux-gui
```

A bare `cargo build` builds only the CLI, so CLI users don't need Qt installed.

---

## What it measures

| Suite | Tests | Units |
|-------|-------|-------|
| **CPU** | BBP-π · SHA-256 · MatMul · LZ4 · Sort, single- **and** multi-threaded | digits/s, MB/s, GFLOPS, M items/s + composite score & speedup |
| **Memory** | STREAM Copy / Scale / Add / Triad | GB/s |
| **Network** | Cloudflare latency (min/avg/max/stddev/jitter), download, upload | ms, Mbps |
| **Storage** | sequential write/read, random 4K read/write latency, `O_DIRECT` | MB/s, µs (p50/p99) |

The exact algorithms, why they're correct, and the subtle bugs that were fixed
(MT core-affinity inheritance, tmpfs masquerading as disk, vectorisation of the
STREAM kernels) are documented in **[docs/methodology.md](docs/methodology.md)**.

---

## Documentation

| Doc | Contents |
|-----|----------|
| [docs/cli.md](docs/cli.md) | Every command, subcommand, and flag |
| [docs/methodology.md](docs/methodology.md) | How each benchmark works and why the numbers are trustworthy |
| [docs/sysinfo.md](docs/sysinfo.md) | Everything `crux info` reports and where it reads it from |
| [docs/architecture.md](docs/architecture.md) | Codebase layout and module responsibilities |
| [docs/packaging.md](docs/packaging.md) | AUR / deb / rpm packaging, host-native build model |

---

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
