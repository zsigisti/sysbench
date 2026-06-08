# Methodology

How each benchmark works, what it reports, and the correctness pitfalls that
were specifically engineered around. The goal is numbers you can trust and
compare across machines.

## General approach

- **Native codegen.** Release builds use LTO, a single codegen unit, and
  `-C target-cpu=native` (via the installer/packagers). The benchmark measures
  what *this* CPU can do with code compiled for it.
- **Warmup + median.** CPU tests run one discarded warmup, then `--runs`
  measured runs, reporting the **median** and **stddev**. A run whose
  stddev/median exceeds 10% is flagged as high-variance (possible thermal
  throttling).
- **Optimiser barriers.** Every kernel writes through `std::ptr::read_volatile`
  or `std::hint::black_box` so LLVM can't delete "useless" work and report
  fake infinite throughput.

## CPU suite

Five workloads, each run **single-threaded (ST)** and **multi-threaded (MT)**.
MT spawns `available_parallelism()` workers. A geometric-mean **composite
score** is computed per mode, and the **speedup** is `composite_mt /
composite_st`.

| Test | What it does | Unit |
|------|--------------|------|
| **BBP-π** | Bailey–Borwein–Plouffe hex digits of π — integer mod-pow heavy | digits/s |
| **SHA-256** | Hash a 1 MiB buffer repeatedly (uses SHA-NI where available) | MB/s |
| **MatMul** | Naïve `ikj` 256×256 `f64` matrix multiply | GFLOPS |
| **LZ4** | Compress a 1 MiB buffer with `lz4_flex` | MB/s |
| **Sort** | `sort_unstable` over 1M `u64`, reshuffled each iteration | M items/s |

### Two correctness pitfalls that were fixed

1. **MT affinity inheritance (severe).** The ST tests pin the *current* thread
   to one core to cut scheduler noise. On Linux a spawned thread **inherits its
   parent's CPU affinity mask** — so once ST had pinned `main` to core 0, every
   MT worker inherited a "core 0 only" mask and all of them piled onto a single
   core. The result: SHA-256, MatMul and Sort showed **~1× MT scaling** on a
   20-core machine. The fix lives in [`src/affinity.rs`](../src/affinity.rs):
   ST pins are wrapped in a `PinGuard` that restores the full CPU set on drop,
   and MT explicitly resets `main` to all cores before spawning. Verified
   scaling on a 20-core 265KF went from ~1× to **17–18×**.

2. **BBP per-digit cost.** `bbp_hex_digit(n)` is `O(n)`. An earlier "fix" started
   each MT thread at a different offset (`i * 1_000_000`), which made high-index
   threads thousands of times slower per digit and tanked MT throughput. All
   threads now start at `n = 0`; there's no shared state, so redundant digits
   are harmless and the measurement reflects raw per-core throughput.

3. **LZ4 measured the allocator, not LZ4.** The compress loop used
   `compress_prepend_size`, which **allocates a fresh `Vec` every iteration**.
   Single-threaded, glibc malloc serialises and the alloc/free dominated,
   pinning ST to ~3 GB/s; multi-threaded, per-thread arenas made allocation
   nearly free, so MT looked like a physically-impossible **~65× scaling** on
   20 cores. Switched to `compress_into` with a reused output buffer: true ST is
   ~30 GB/s and MT scaling is a sane ~7× (memory-scan bound, like Sort). The
   composite-score divisor for LZ4 was rescaled (1000 → 10000) to match.

> **Reading the speedup.** Expect near-linear scaling for compute-bound tests
> (SHA, MatMul, BBP) and **sub-linear** scaling for memory-bound ones (Sort
> thrashes ~8 MB per thread, so it saturates the memory subsystem well before
> core count). That's real hardware behaviour, not a bug.

## Memory suite (STREAM)

Classic STREAM kernels over three 256 MiB `f64` arrays, reporting GB/s:

| Kernel | Operation | Streams |
|--------|-----------|---------|
| Copy | `b = a` | 2 |
| Scale | `b = k * c` | 2 |
| Add | `c = a + b` | 3 |
| Triad | `a = b + k * c` | 3 |

**Vectorisation fix.** Written with `arr[i]` indexing, the 3-array Add/Triad
loops kept bounds checks that LLVM wouldn't always hoist, which **blocked
auto-vectorisation** and made them ~3× slower than Copy. The kernels now use
iterator `zip`, which is provably in-bounds and vectorises to SIMD — Add/Triad
now land within ~10% of Copy, as STREAM expects. A post-loop `black_box(&out)`
defeats dead-store elimination (otherwise a later kernel overwriting the same
array makes the stores dead and the loop vanishes, yielding fake TB/s).

This is single-threaded bandwidth — a useful per-core figure, typically below
the chip's aggregate peak (which needs many threads to reach).

## Network suite

Uses Cloudflare's public speed-test endpoints (`speed.cloudflare.com`):

- **Latency** — 20 zero-byte requests; reports min / avg / max / stddev and
  mean **jitter** (avg absolute delta between consecutive samples).
- **Download** — `--streams` parallel readers of a 100 MiB object, with a 5 s
  warmup then 10 s measured window. Reported in Mbps.
- **Upload** — POST 50 MiB and time it; Mbps.

Each leg fails independently with its error captured, so one failure doesn't
abort the rest.

## Storage suite

A scratch file sized to `min(2 × RAM, 4 GiB)`, clamped to available free space
(uses at most half of free space, leaves 10% headroom, aligns to 4 MiB, and
errors cleanly below 256 MiB).

| Metric | Method |
|--------|--------|
| Seq write | Write the whole file in 4 MiB chunks, then `fsync`; MB/s |
| Seq read | Drop caches if root, read it back; MB/s |
| Rand 4K read | 1000 random-offset 4 KiB reads; p50 / p99 latency in µs |
| Rand 4K write | 1000 random-offset 4 KiB writes + `fdatasync`; p50 / p99 µs |

**On Linux** the file is opened with `O_DIRECT` and a `posix_memalign`'d,
4096-aligned buffer to bypass the page cache. On other platforms it falls back
to buffered I/O (and flags the read as likely cached).

**tmpfs detection.** `O_DIRECT` is a no-op on RAM-backed filesystems, so a
scratch file on `/tmp` (tmpfs on most systemd distros) yields fantasy numbers
(~16 GB/s, 1 µs latency). `crux` `statfs`-checks the scratch directory and, if
it's tmpfs/ramfs, prints a prominent warning and tells you to re-run with
`--dir` pointing at a real disk. The default scratch dir is the current working
directory precisely to avoid silently landing on `/tmp`.
