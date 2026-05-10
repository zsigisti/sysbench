// Network — Cloudflare speed test endpoints.

use serde::Serialize;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::stats;

#[derive(Debug, Clone, Serialize)]
pub struct LatencyResult {
    pub min_ms: f64,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub stddev_ms: f64,
    pub jitter_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetResults {
    pub latency: Result<LatencyResult, String>,
    pub download_mbps: Result<f64, String>,
    pub upload_mbps: Result<f64, String>,
}

fn cf_get(url: &str) -> Result<ureq::Response, Box<dyn std::error::Error>> {
    Ok(ureq::get(url)
        .set("Origin", "https://speed.cloudflare.com")
        .set("Referer", "https://speed.cloudflare.com/")
        .call()?)
}

fn err_to_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------- Latency ----------

fn measure_latency() -> Result<LatencyResult, Box<dyn std::error::Error>> {
    let url = "https://speed.cloudflare.com/__down?bytes=0";
    let mut times = Vec::with_capacity(20);
    for _ in 0..20 {
        let start = Instant::now();
        let resp = cf_get(url)?;
        let mut reader = resp.into_reader();
        let mut sink = [0u8; 1024];
        loop {
            match reader.read(&mut sink) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => return Err(Box::new(e)),
            }
        }
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    if times.is_empty() {
        return Err("no samples".into());
    }
    let min_ms = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_ms = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg_ms = stats::mean(&times);
    let stddev_ms = stats::stddev(&times);
    let mut jitter_sum = 0.0f64;
    let mut count = 0usize;
    for w in times.windows(2) {
        jitter_sum += (w[1] - w[0]).abs();
        count += 1;
    }
    let jitter_ms = if count > 0 {
        jitter_sum / (count as f64)
    } else {
        0.0
    };
    Ok(LatencyResult {
        min_ms,
        avg_ms,
        max_ms,
        stddev_ms,
        jitter_ms,
    })
}

// ---------- Download ----------

fn measure_download(streams: usize) -> Result<f64, Box<dyn std::error::Error>> {
    let url = "https://speed.cloudflare.com/__down?bytes=104857600";
    let measuring = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = Arc::new(AtomicU64::new(0));

    let mut handles = Vec::with_capacity(streams);
    for _ in 0..streams {
        let measuring = measuring.clone();
        let stop = stop.clone();
        let bytes = bytes.clone();
        let url = url.to_string();
        handles.push(thread::spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            while !stop.load(Ordering::Relaxed) {
                let resp = match cf_get(&url) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let mut reader = resp.into_reader();
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if measuring.load(Ordering::Relaxed) {
                                bytes.fetch_add(n as u64, Ordering::Relaxed);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }));
    }

    // Warm up 5s
    thread::sleep(Duration::from_secs(5));
    measuring.store(true, Ordering::Relaxed);
    let t0 = Instant::now();
    thread::sleep(Duration::from_secs(10));
    measuring.store(false, Ordering::Relaxed);
    let secs = t0.elapsed().as_secs_f64().max(1e-9);
    stop.store(true, Ordering::Relaxed);

    for h in handles {
        let _ = h.join();
    }

    let total = bytes.load(Ordering::Relaxed);
    Ok((total as f64 * 8.0) / (secs * 1_000_000.0))
}

// ---------- Upload ----------

fn measure_upload() -> Result<f64, Box<dyn std::error::Error>> {
    let url = "https://speed.cloudflare.com/__up";
    let size: usize = 50 * 1024 * 1024;
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

// ---------- Public runner ----------

pub fn run(streams: usize) -> NetResults {
    let latency = measure_latency().map_err(err_to_string);
    let download_mbps = measure_download(streams).map_err(err_to_string);
    let upload_mbps = measure_upload().map_err(err_to_string);
    NetResults {
        latency,
        download_mbps,
        upload_mbps,
    }
}
