// CPU benchmarks: BBP-π, SHA-256, MatMul, LZ4, Sort.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::stats;

// ---------- TestScore ----------

#[derive(Debug, Clone, Serialize)]
pub struct TestScore {
    pub median: f64,
    pub stddev: f64,
    pub unit: String,
    pub high_variance: bool,
}

impl TestScore {
    fn from_runs(mut runs: Vec<f64>, unit: &str) -> Self {
        let s = stats::stddev(&runs);
        let m = stats::median(&mut runs);
        let high_variance = m > 0.0 && (s / m) > 0.10;
        TestScore {
            median: m,
            stddev: s,
            unit: unit.to_string(),
            high_variance,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuResults {
    pub threads: usize,
    pub bbp_st: TestScore,
    pub sha256_st: TestScore,
    pub matmul_st: TestScore,
    pub lz4_st: TestScore,
    pub sort_st: TestScore,
    pub composite_st: f64,

    pub bbp_mt: TestScore,
    pub sha256_mt: TestScore,
    pub matmul_mt: TestScore,
    pub lz4_mt: TestScore,
    pub sort_mt: TestScore,
    pub composite_mt: f64,

    pub speedup: f64,
}

// ============================================================
// BBP-π (Bailey–Borwein–Plouffe)
// ============================================================

fn modpow(base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus <= 1 {
        return 0;
    }
    let m = modulus as u128;
    let mut result: u128 = 1;
    let mut b: u128 = (base as u128) % m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = (result * b) % m;
        }
        exp >>= 1;
        b = (b * b) % m;
    }
    result as u64
}

fn series(j: u64, n: u64) -> f64 {
    let mut s = 0.0f64;
    for k in 0..=n {
        let denom = 8 * k + j;
        let r = modpow(16, n - k, denom);
        s += (r as f64) / (denom as f64);
        s -= s.floor();
    }
    let mut k = n + 1;
    let mut p = 1.0f64 / 16.0;
    while p > 1e-17 {
        s += p / ((8 * k + j) as f64);
        p /= 16.0;
        k += 1;
    }
    s - s.floor()
}

fn bbp_hex_digit(n: u64) -> u8 {
    let x = 4.0 * series(1, n) - 2.0 * series(4, n) - series(5, n) - series(6, n);
    let mut frac = x - x.floor();
    if frac < 0.0 {
        frac += 1.0;
    }
    ((frac * 16.0) as u32 & 0xF) as u8
}

/// Pin current thread to a specific core if available.
fn pin_to_core(idx: usize) {
    if let Some(ids) = core_affinity::get_core_ids() {
        if let Some(&id) = ids.get(idx) {
            let _ = core_affinity::set_for_current(id);
        }
    }
}

// ---------- BBP single ----------
fn bbp_single_run(dur: Duration) -> f64 {
    pin_to_core(0);
    let start = Instant::now();
    let mut n: u64 = 0;
    while start.elapsed() < dur {
        for _ in 0..50 {
            let d = bbp_hex_digit(n);
            unsafe {
                std::ptr::read_volatile(&d);
            }
            n += 1;
        }
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    (n as f64) / secs
}

fn bbp_multi_run(dur: Duration, threads: usize) -> f64 {
    let counter = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::with_capacity(threads);

    let core_ids = core_affinity::get_core_ids().unwrap_or_default();

    for i in 0..threads {
        let counter = counter.clone();
        let stop = stop.clone();
        let pin = core_ids.get(i).copied();
        handles.push(thread::spawn(move || {
            if let Some(id) = pin {
                let _ = core_affinity::set_for_current(id);
            }
            while !stop.load(Ordering::Relaxed) {
                let start_n = counter.fetch_add(50, Ordering::Relaxed);
                for k in 0..50u64 {
                    let d = bbp_hex_digit(start_n + k);
                    unsafe {
                        std::ptr::read_volatile(&d);
                    }
                }
            }
        }));
    }

    let start = Instant::now();
    thread::sleep(dur);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        let _ = h.join();
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    (counter.load(Ordering::Relaxed) as f64) / secs
}

// ============================================================
// SHA-256 — hash 1 MiB blocks. Unit MB/s.
// ============================================================

const SHA_BLOCK: usize = 1024 * 1024;

fn sha_single_run(dur: Duration) -> f64 {
    pin_to_core(0);
    let buf = vec![0xA5u8; SHA_BLOCK];
    let start = Instant::now();
    let mut iters: u64 = 0;
    while start.elapsed() < dur {
        let mut h = Sha256::new();
        h.update(&buf);
        let out = h.finalize();
        unsafe {
            std::ptr::read_volatile(&out[0]);
        }
        iters += 1;
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    let bytes = iters as f64 * SHA_BLOCK as f64;
    bytes / 1.0e6 / secs
}

fn sha_multi_run(dur: Duration, threads: usize) -> f64 {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let mut handles = Vec::with_capacity(threads);
    let start = Instant::now();
    for i in 0..threads {
        let pin = core_ids.get(i).copied();
        handles.push(thread::spawn(move || -> u64 {
            if let Some(id) = pin {
                let _ = core_affinity::set_for_current(id);
            }
            let buf = vec![0xA5u8; SHA_BLOCK];
            let t0 = Instant::now();
            let mut iters: u64 = 0;
            while t0.elapsed() < dur {
                let mut h = Sha256::new();
                h.update(&buf);
                let out = h.finalize();
                unsafe {
                    std::ptr::read_volatile(&out[0]);
                }
                iters += 1;
            }
            iters
        }));
    }
    let mut total: u64 = 0;
    for h in handles {
        total += h.join().unwrap_or(0);
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    let bytes = total as f64 * SHA_BLOCK as f64;
    bytes / 1.0e6 / secs
}

// ============================================================
// Matrix multiply — naive f64, N=256, ikj order. Unit GFLOPS.
// ============================================================

const MATMUL_N: usize = 256;

fn matmul_once(a: &[f64], b: &[f64], c: &mut [f64]) {
    let n = MATMUL_N;
    for x in c.iter_mut() {
        *x = 0.0;
    }
    for i in 0..n {
        for k in 0..n {
            let aik = a[i * n + k];
            for j in 0..n {
                c[i * n + j] += aik * b[k * n + j];
            }
        }
    }
}

fn matmul_single_run(dur: Duration) -> f64 {
    pin_to_core(0);
    let n = MATMUL_N;
    let a = vec![1.0001f64; n * n];
    let b = vec![0.9999f64; n * n];
    let mut c = vec![0.0f64; n * n];
    let start = Instant::now();
    let mut iters: u64 = 0;
    while start.elapsed() < dur {
        matmul_once(&a, &b, &mut c);
        unsafe {
            std::ptr::read_volatile(&c[0]);
        }
        iters += 1;
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    let flops = iters as f64 * 2.0 * (n as f64).powi(3);
    flops / secs / 1.0e9
}

fn matmul_multi_run(dur: Duration, threads: usize) -> f64 {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let mut handles = Vec::with_capacity(threads);
    for i in 0..threads {
        let pin = core_ids.get(i).copied();
        handles.push(thread::spawn(move || -> f64 {
            if let Some(id) = pin {
                let _ = core_affinity::set_for_current(id);
            }
            let n = MATMUL_N;
            let a = vec![1.0001f64; n * n];
            let b = vec![0.9999f64; n * n];
            let mut c = vec![0.0f64; n * n];
            let t0 = Instant::now();
            let mut iters: u64 = 0;
            while t0.elapsed() < dur {
                matmul_once(&a, &b, &mut c);
                unsafe {
                    std::ptr::read_volatile(&c[0]);
                }
                iters += 1;
            }
            let secs = t0.elapsed().as_secs_f64().max(1e-9);
            (iters as f64 * 2.0 * (n as f64).powi(3)) / secs / 1.0e9
        }));
    }
    let mut total = 0.0f64;
    for h in handles {
        total += h.join().unwrap_or(0.0);
    }
    total
}

// ============================================================
// LZ4 — compress 1 MiB semi-compressible buffer. Unit MB/s.
// ============================================================

const LZ4_BLOCK: usize = 1024 * 1024;

fn make_lz4_buf() -> Vec<u8> {
    let mut buf = vec![0u8; LZ4_BLOCK];
    let mut lcg: u64 = 0x9E3779B97F4A7C15;
    for x in buf.iter_mut() {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *x = ((lcg >> 33) as u8) & 0x0F;
    }
    buf
}

fn lz4_single_run(dur: Duration) -> f64 {
    pin_to_core(0);
    let data = make_lz4_buf();
    let start = Instant::now();
    let mut iters: u64 = 0;
    while start.elapsed() < dur {
        let out = lz4_flex::compress_prepend_size(&data);
        unsafe {
            std::ptr::read_volatile(&out[0]);
        }
        iters += 1;
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    let bytes = iters as f64 * LZ4_BLOCK as f64;
    bytes / 1.0e6 / secs
}

fn lz4_multi_run(dur: Duration, threads: usize) -> f64 {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let mut handles = Vec::with_capacity(threads);
    let start = Instant::now();
    for i in 0..threads {
        let pin = core_ids.get(i).copied();
        handles.push(thread::spawn(move || -> u64 {
            if let Some(id) = pin {
                let _ = core_affinity::set_for_current(id);
            }
            let data = make_lz4_buf();
            let t0 = Instant::now();
            let mut iters: u64 = 0;
            while t0.elapsed() < dur {
                let out = lz4_flex::compress_prepend_size(&data);
                unsafe {
                    std::ptr::read_volatile(&out[0]);
                }
                iters += 1;
            }
            iters
        }));
    }
    let mut total: u64 = 0;
    for h in handles {
        total += h.join().unwrap_or(0);
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    let bytes = total as f64 * LZ4_BLOCK as f64;
    bytes / 1.0e6 / secs
}

// ============================================================
// Sort — sort_unstable on Vec<u64> of 1M elements. Unit M items/s.
// ============================================================

const SORT_N: usize = 1_000_000;

fn xorshift64(s: &mut u64) -> u64 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *s = x;
    x
}

fn shuffle(buf: &mut [u64], seed: &mut u64) {
    for x in buf.iter_mut() {
        *x = xorshift64(seed);
    }
}

fn sort_single_run(dur: Duration) -> f64 {
    pin_to_core(0);
    let mut buf = vec![0u64; SORT_N];
    let mut seed: u64 = 0xDEADBEEFCAFEBABE;
    let start = Instant::now();
    let mut iters: u64 = 0;
    while start.elapsed() < dur {
        shuffle(&mut buf, &mut seed);
        buf.sort_unstable();
        unsafe {
            std::ptr::read_volatile(&buf[0]);
        }
        iters += 1;
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    (iters as f64 * SORT_N as f64) / 1.0e6 / secs
}

fn sort_multi_run(dur: Duration, threads: usize) -> f64 {
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let mut handles = Vec::with_capacity(threads);
    let start = Instant::now();
    for i in 0..threads {
        let pin = core_ids.get(i).copied();
        handles.push(thread::spawn(move || -> u64 {
            if let Some(id) = pin {
                let _ = core_affinity::set_for_current(id);
            }
            let mut buf = vec![0u64; SORT_N];
            let mut seed: u64 = 0xDEADBEEFCAFEBABE ^ (i as u64).wrapping_mul(0x9E3779B97F4A7C15);
            if seed == 0 {
                seed = 1;
            }
            let t0 = Instant::now();
            let mut iters: u64 = 0;
            while t0.elapsed() < dur {
                shuffle(&mut buf, &mut seed);
                buf.sort_unstable();
                unsafe {
                    std::ptr::read_volatile(&buf[0]);
                }
                iters += 1;
            }
            iters
        }));
    }
    let mut total: u64 = 0;
    for h in handles {
        total += h.join().unwrap_or(0);
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    (total as f64 * SORT_N as f64) / 1.0e6 / secs
}

// ============================================================
// Runner
// ============================================================

fn run_bench<F: FnMut() -> f64>(label: &str, runs: usize, mut f: F) -> Vec<f64> {
    // 1 warmup
    let _ = f();
    let mut out = Vec::with_capacity(runs);
    for i in 0..runs {
        let v = f();
        println!("    {} run {}/{}: {:.2}", label, i + 1, runs, v);
        out.push(v);
    }
    out
}

pub fn run(dur: Duration, runs: usize) -> CpuResults {
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);

    println!("[CPU] Single-threaded benchmarks");

    println!("  BBP-π (digits/s)...");
    let bbp_st_runs = run_bench("BBP-ST", runs, || bbp_single_run(dur));
    let bbp_st = TestScore::from_runs(bbp_st_runs, "digits/s");

    println!("  SHA-256 (MB/s)...");
    let sha_st_runs = run_bench("SHA-ST", runs, || sha_single_run(dur));
    let sha256_st = TestScore::from_runs(sha_st_runs, "MB/s");

    println!("  MatMul (GFLOPS)...");
    let mat_st_runs = run_bench("MAT-ST", runs, || matmul_single_run(dur));
    let matmul_st = TestScore::from_runs(mat_st_runs, "GFLOPS");

    println!("  LZ4 (MB/s)...");
    let lz4_st_runs = run_bench("LZ4-ST", runs, || lz4_single_run(dur));
    let lz4_st = TestScore::from_runs(lz4_st_runs, "MB/s");

    println!("  Sort (M items/s)...");
    let sort_st_runs = run_bench("SORT-ST", runs, || sort_single_run(dur));
    let sort_st = TestScore::from_runs(sort_st_runs, "M items/s");

    let composite_st = stats::geomean(&[
        bbp_st.median / 5000.0,
        sha256_st.median / 500.0,
        matmul_st.median / 10.0,
        lz4_st.median / 1000.0,
        sort_st.median / 50.0,
    ]) * 1000.0;

    println!();
    println!("[CPU] Multi-threaded benchmarks ({} threads)", threads);

    println!("  BBP-π (digits/s)...");
    let bbp_mt_runs = run_bench("BBP-MT", runs, || bbp_multi_run(dur, threads));
    let bbp_mt = TestScore::from_runs(bbp_mt_runs, "digits/s");

    println!("  SHA-256 (MB/s)...");
    let sha_mt_runs = run_bench("SHA-MT", runs, || sha_multi_run(dur, threads));
    let sha256_mt = TestScore::from_runs(sha_mt_runs, "MB/s");

    println!("  MatMul (GFLOPS)...");
    let mat_mt_runs = run_bench("MAT-MT", runs, || matmul_multi_run(dur, threads));
    let matmul_mt = TestScore::from_runs(mat_mt_runs, "GFLOPS");

    println!("  LZ4 (MB/s)...");
    let lz4_mt_runs = run_bench("LZ4-MT", runs, || lz4_multi_run(dur, threads));
    let lz4_mt = TestScore::from_runs(lz4_mt_runs, "MB/s");

    println!("  Sort (M items/s)...");
    let sort_mt_runs = run_bench("SORT-MT", runs, || sort_multi_run(dur, threads));
    let sort_mt = TestScore::from_runs(sort_mt_runs, "M items/s");

    let composite_mt = stats::geomean(&[
        bbp_mt.median / 5000.0,
        sha256_mt.median / 500.0,
        matmul_mt.median / 10.0,
        lz4_mt.median / 1000.0,
        sort_mt.median / 50.0,
    ]) * 1000.0;

    let speedup = if composite_st > 0.0 {
        composite_mt / composite_st
    } else {
        0.0
    };

    CpuResults {
        threads,
        bbp_st,
        sha256_st,
        matmul_st,
        lz4_st,
        sort_st,
        composite_st,
        bbp_mt,
        sha256_mt,
        matmul_mt,
        lz4_mt,
        sort_mt,
        composite_mt,
        speedup,
    }
}
