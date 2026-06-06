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

    // black_box() barriers around each kernel prevent dead-store elimination:
    // without them, a later kernel that overwrites an array makes the earlier
    // kernel's stores dead and LLVM deletes the whole loop (giving fake TB/s).

    // Copy: b[i] = a[i]
    let mut copy_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        black_box(&a);
        let t0 = Instant::now();
        for i in 0..N {
            b[i] = a[i];
        }
        black_box(&b);
        let secs = t0.elapsed().as_secs_f64();
        copy_runs.push(gbs(bytes_2n, secs));
    }

    // Scale: b[i] = scalar * c[i]
    let mut scale_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        black_box(&c);
        let t0 = Instant::now();
        for i in 0..N {
            b[i] = SCALAR * c[i];
        }
        black_box(&b);
        let secs = t0.elapsed().as_secs_f64();
        scale_runs.push(gbs(bytes_2n, secs));
    }

    // Add: c[i] = a[i] + b[i]
    let mut add_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        black_box(&a);
        black_box(&b);
        let t0 = Instant::now();
        for i in 0..N {
            c[i] = a[i] + b[i];
        }
        black_box(&c);
        let secs = t0.elapsed().as_secs_f64();
        add_runs.push(gbs(bytes_3n, secs));
    }

    // Triad: a[i] = b[i] + scalar * c[i]
    let mut triad_runs = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        black_box(&b);
        black_box(&c);
        let t0 = Instant::now();
        for i in 0..N {
            a[i] = b[i] + SCALAR * c[i];
        }
        black_box(&a);
        let secs = t0.elapsed().as_secs_f64();
        triad_runs.push(gbs(bytes_3n, secs));
    }

    MemResults {
        copy_gbs: stats::median(&mut copy_runs),
        scale_gbs: stats::median(&mut scale_runs),
        add_gbs: stats::median(&mut add_runs),
        triad_gbs: stats::median(&mut triad_runs),
    }
}
