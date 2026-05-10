// sysbench — CPU / Network / Storage benchmark in one file.
//
// CPU:     hex digits of π via BBP formula (parallel-friendly)
// Network: Cloudflare speed-test endpoints
// Storage: write + read a 1 GB file in $TMPDIR

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================
// CPU — Bailey–Borwein–Plouffe formula for hex digits of π
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

// Fractional part of 16^n * Σ 1/(16^k * (8k+j))
fn series(j: u64, n: u64) -> f64 {
    let mut s = 0.0f64;
    // First sum: k = 0..=n, using modular arithmetic
    for k in 0..=n {
        let denom = 8 * k + j;
        let r = modpow(16, n - k, denom);
        s += (r as f64) / (denom as f64);
        s -= s.floor();
    }
    // Tail: k > n, terms shrink by 1/16 each time
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

fn cpu_single(dur: Duration) -> u64 {
    let start = Instant::now();
    let mut n = 0u64;
    // Batch to amortise the time check
    while start.elapsed() < dur {
        for _ in 0..50 {
            // black_box not in stable std without nightly; assign to a volatile-ish sink
            let d = bbp_hex_digit(n);
            // prevent the optimiser from eliminating the call
            unsafe {
                std::ptr::read_volatile(&d);
            }
            n += 1;
        }
    }
    n
}

fn cpu_multi(dur: Duration, threads: usize) -> u64 {
    let counter = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::with_capacity(threads);

    for _ in 0..threads {
        let counter = counter.clone();
        let stop = stop.clone();
        handles.push(thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let start_n = counter.fetch_add(50, Ordering::Relaxed);
                for i in 0..50u64 {
                    let d = bbp_hex_digit(start_n + i);
                    unsafe {
                        std::ptr::read_volatile(&d);
                    }
                }
            }
        }));
    }

    thread::sleep(dur);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        let _ = h.join();
    }
    counter.load(Ordering::Relaxed)
}

fn avg_three<F: FnMut() -> u64>(label: &str, mut f: F) -> u64 {
    let mut total = 0u64;
    for i in 1..=3 {
        let r = f();
        println!("    {} run {}: {} digits", label, i, r);
        total += r;
    }
    total / 3
}

// ============================================================
// Network — Cloudflare speed test
// ============================================================

fn cf_get(url: &str) -> Result<ureq::Response, Box<dyn std::error::Error>> {
    Ok(ureq::get(url)
        .set("Origin", "https://speed.cloudflare.com")
        .set("Referer", "https://speed.cloudflare.com/")
        .call()?)
}

fn net_latency() -> Result<f64, Box<dyn std::error::Error>> {
    let url = "https://speed.cloudflare.com/__down?bytes=0";
    let mut times = Vec::with_capacity(10);
    for _ in 0..10 {
        let start = Instant::now();
        let resp = cf_get(url)?;
        let mut buf = vec![0u8; 1024];
        let mut reader = resp.into_reader();
        while reader.read(&mut buf)? > 0 {}
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Ok(times[times.len() / 2]) // median
}

fn net_download() -> Result<f64, Box<dyn std::error::Error>> {
    let url = "https://speed.cloudflare.com/__down?bytes=104857600";
    let resp = cf_get(url)?;
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let start = Instant::now();
    let mut total: u64 = 0;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => total += n as u64,
            Err(e) => return Err(Box::new(e)),
        }
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    Ok((total as f64 * 8.0) / (secs * 1_000_000.0))
}

fn net_upload() -> Result<f64, Box<dyn std::error::Error>> {
    let url = "https://speed.cloudflare.com/__up";
    let size: usize = 52_428_800; // 50 MiB
    let data = vec![0u8; size];
    let start = Instant::now();
    let _ = ureq::post(url)
        .set("Content-Type", "application/octet-stream")
        .set("Origin", "https://speed.cloudflare.com")
        .set("Referer", "https://speed.cloudflare.com/")
        .send_bytes(&data)?;
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    Ok((size as f64 * 8.0) / (secs * 1_000_000.0))
}


// ============================================================
// Storage — write + read a big file
// ============================================================

fn storage_test() -> std::io::Result<(f64, f64)> {
    let mut path: PathBuf = std::env::temp_dir();
    path.push("sysbench_scratch.bin");

    let total: u64 = 1024 * 1024 * 1024; // 1 GiB
    let chunk_size = 4 * 1024 * 1024;    // 4 MiB
    let chunk = vec![0xA5u8; chunk_size];

    // Write pass
    let start = Instant::now();
    {
        let mut f = std::fs::File::create(&path)?;
        let mut written: u64 = 0;
        while written < total {
            f.write_all(&chunk)?;
            written += chunk_size as u64;
        }
        f.sync_all()?; // flush to physical device (best effort)
    }
    let write_secs = start.elapsed().as_secs_f64().max(1e-9);
    let write_mbs = (total as f64 / (1024.0 * 1024.0)) / write_secs;

    // Read pass — note: the OS page cache will likely make this fast.
    let start = Instant::now();
    {
        let mut f = std::fs::File::open(&path)?;
        let mut buf = vec![0u8; chunk_size];
        loop {
            match f.read(&mut buf)? {
                0 => break,
                _ => {}
            }
        }
    }
    let read_secs = start.elapsed().as_secs_f64().max(1e-9);
    let read_mbs = (total as f64 / (1024.0 * 1024.0)) / read_secs;

    let _ = std::fs::remove_file(&path);
    Ok((write_mbs, read_mbs))
}

// ============================================================
// main
// ============================================================

fn main() {
    println!("===========================================");
    println!("  sysbench — Rust system benchmark");
    println!("===========================================\n");

    // ---------- CPU ----------
    println!("[1] CPU — π hex digits via BBP (10 s per run, avg of 3)\n");
    let cores = thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(1);

    println!("  Single-threaded:");
    let st = avg_three("ST", || cpu_single(Duration::from_secs(10)));
    println!("    => Single-threaded score: {} digits (avg)\n", st);

    println!("  Multi-threaded ({} threads):", cores);
    let mt = avg_three("MT", || cpu_multi(Duration::from_secs(10), cores));
    println!("    => Multi-threaded score: {} digits (avg)\n", mt);

    let speedup = mt as f64 / st.max(1) as f64;
    println!("  Speedup (MT / ST): {:.2}×\n", speedup);

    // ---------- Network ----------
    println!("[2] Network — Cloudflare speed test\n");
    print!("  Latency (10 pings, median)... ");
    std::io::stdout().flush().ok();
    match net_latency() {
        Ok(v) => println!("{:.2} ms", v),
        Err(e) => println!("failed: {}", e),
    }
    print!("  Download (100 MB)...          ");
    std::io::stdout().flush().ok();
    match net_download() {
        Ok(v) => println!("{:.2} Mbps", v),
        Err(e) => println!("failed: {}", e),
    }
    print!("  Upload   (50 MB)...           ");
    std::io::stdout().flush().ok();
    match net_upload() {
        Ok(v) => println!("{:.2} Mbps", v),
        Err(e) => println!("failed: {}", e),
    }
    println!();

    // ---------- Storage ----------
    println!("[3] Storage — 1 GB file in {}", std::env::temp_dir().display());
    print!("  Working... ");
    std::io::stdout().flush().ok();
    match storage_test() {
        Ok((w, r)) => {
            println!("done");
            println!("    Write: {:>8.2} MB/s", w);
            println!("    Read:  {:>8.2} MB/s  (likely served from OS cache)", r);
        }
        Err(e) => println!("failed: {}", e),
    }

    println!("\n===========================================");
}
