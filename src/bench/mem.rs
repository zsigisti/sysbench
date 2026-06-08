// Memory bandwidth — STREAM-style benchmark.
//
// 3 arrays of f64 with 32M elements each (256 MiB per array).
// 5 iterations per kernel, median GB/s reported.

use serde::Serialize;
use std::hint::black_box;
use std::time::Instant;

use crate::stats;

const N: usize = 32 * 1024 * 1024; // 32M elements -> 256 MiB
const ITERS: usize = 5;
const SCALAR: f64 = 3.0;

#[derive(Debug, Clone, Serialize)]
pub struct MemResults {
    pub copy_gbs: f64,
    pub scale_gbs: f64,
    pub add_gbs: f64,
    pub triad_gbs: f64,
}

fn gbs(bytes: f64, secs: f64) -> f64 {
    bytes / 1.0e9 / secs.max(1e-9)
}

pub fn run() -> MemResults {
    let mut a = vec![1.0f64; N];
    let mut b = vec![2.0f64; N];
    let mut c = vec![0.5f64; N];

    let bytes_2n = 2.0 * N as f64 * 8.0;
    let bytes_3n = 3.0 * N as f64 * 8.0;

    // The kernels use iterator `zip` rather than `arr[i]` indexing on purpose:
    // indexing three slices (Add/Triad) leaves bounds checks LLVM won't always
    // hoist, which blocks auto-vectorisation and made Add/Triad run ~3x slower
    // than Copy. `zip` is provably in-bounds, so the loops vectorise to SIMD.
    //
    // The post-loop `black_box(&out)` marks the written array as observed,
    // defeating dead-store elimination — without it a later kernel that
    // overwrites the same array makes the stores dead and LLVM deletes the loop
    // entirely (yielding fake TB/s readings).

    // Copy: b = a
    let mut copy_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        for (d, &s) in b.iter_mut().zip(a.iter()) {
            *d = s;
        }
        black_box(&b);
        copy_runs.push(gbs(bytes_2n, t0.elapsed().as_secs_f64()));
    }

    // Scale: b = scalar * c
    let mut scale_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        for (d, &s) in b.iter_mut().zip(c.iter()) {
            *d = SCALAR * s;
        }
        black_box(&b);
        scale_runs.push(gbs(bytes_2n, t0.elapsed().as_secs_f64()));
    }

    // Add: c = a + b
    let mut add_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        for (d, (&x, &y)) in c.iter_mut().zip(a.iter().zip(b.iter())) {
            *d = x + y;
        }
        black_box(&c);
        add_runs.push(gbs(bytes_3n, t0.elapsed().as_secs_f64()));
    }

    // Triad: a = b + scalar * c
    let mut triad_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        for (d, (&x, &y)) in a.iter_mut().zip(b.iter().zip(c.iter())) {
            *d = x + SCALAR * y;
        }
        black_box(&a);
        triad_runs.push(gbs(bytes_3n, t0.elapsed().as_secs_f64()));
    }

    MemResults {
        copy_gbs: stats::median(&mut copy_runs),
        scale_gbs: stats::median(&mut scale_runs),
        add_gbs: stats::median(&mut add_runs),
        triad_gbs: stats::median(&mut triad_runs),
    }
}
