// Storage benchmarks — sequential & random I/O.
//
// On Linux: uses O_DIRECT with a 4096-aligned buffer to bypass the page cache.
// On other platforms: regular buffered I/O.

use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::stats;

#[derive(Debug, Clone, Serialize)]
pub struct DiskResults {
    pub file_size_mib: u64,
    pub seq_write_mbs: f64,
    pub seq_read_mbs: f64,
    pub seq_read_cached: bool,
    pub rand_read_p50_us: f64,
    pub rand_read_p99_us: f64,
    pub rand_write_p50_us: f64,
    pub rand_write_p99_us: f64,
}

const CHUNK: usize = 4 * 1024 * 1024; // 4 MiB
const RAND_OPS: usize = 1000;
const RAND_BLOCK: usize = 4096;

fn choose_file_size(ram_mib: u64) -> u64 {
    let target = (ram_mib * 2).max(2048) * 1024 * 1024;
    target.min(4 * 1024 * 1024 * 1024)
}

// ---------- Cleanup guard ----------

struct TempPath(PathBuf);

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// ---------- xorshift64 for random offsets ----------

fn xorshift64(s: &mut u64) -> u64 {
    let mut x = *s;
    if x == 0 {
        x = 0xDEADBEEFCAFEBABE;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *s = x;
    x
}

// ============================================================
// Linux-only O_DIRECT helpers + aligned buffer.
// ============================================================

#[cfg(target_os = "linux")]
mod linux_io {
    use super::*;
    use std::os::unix::fs::OpenOptionsExt;

    pub fn open_direct_write(path: &Path) -> std::io::Result<File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .custom_flags(libc::O_DIRECT)
            .open(path)
    }

    pub fn open_direct_read(path: &Path) -> std::io::Result<File> {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECT)
            .open(path)
    }

    pub fn open_direct_rw(path: &Path) -> std::io::Result<File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_DIRECT)
            .open(path)
    }

    pub struct AlignedBuf {
        ptr: *mut u8,
        len: usize,
    }

    // The pointer is owned exclusively by this struct; safe to send.
    unsafe impl Send for AlignedBuf {}
    unsafe impl Sync for AlignedBuf {}

    impl AlignedBuf {
        pub fn new(size: usize, align: usize) -> std::io::Result<Self> {
            let mut raw: *mut libc::c_void = std::ptr::null_mut();
            let rc = unsafe { libc::posix_memalign(&mut raw, align, size) };
            if rc != 0 {
                return Err(std::io::Error::from_raw_os_error(rc));
            }
            // Zero-init.
            unsafe {
                std::ptr::write_bytes(raw as *mut u8, 0xA5, size);
            }
            Ok(AlignedBuf {
                ptr: raw as *mut u8,
                len: size,
            })
        }

        pub fn as_slice(&self) -> &[u8] {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }

        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
        }
    }

    impl Drop for AlignedBuf {
        fn drop(&mut self) {
            unsafe {
                libc::free(self.ptr as *mut libc::c_void);
            }
        }
    }

    pub fn try_drop_caches() {
        // Only works as root. Errors are silently ignored.
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .open("/proc/sys/vm/drop_caches")
        {
            let _ = f.write_all(b"3\n");
        }
    }
}

// ============================================================
// Sequential write/read
// ============================================================

#[cfg(target_os = "linux")]
fn seq_write(path: &Path, file_size: u64) -> std::io::Result<f64> {
    use linux_io::*;
    let mut f = open_direct_write(path)?;
    let mut buf = AlignedBuf::new(CHUNK, 4096)?;
    let slice = buf.as_mut_slice();
    let start = Instant::now();
    let mut written: u64 = 0;
    while written < file_size {
        f.write_all(slice)?;
        written += CHUNK as u64;
    }
    f.sync_all()?;
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    Ok((file_size as f64 / 1.0e6) / secs)
}

#[cfg(not(target_os = "linux"))]
fn seq_write(path: &Path, file_size: u64) -> std::io::Result<f64> {
    let mut f = File::create(path)?;
    let buf = vec![0xA5u8; CHUNK];
    let start = Instant::now();
    let mut written: u64 = 0;
    while written < file_size {
        f.write_all(&buf)?;
        written += CHUNK as u64;
    }
    f.sync_all()?;
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    Ok((file_size as f64 / 1.0e6) / secs)
}

#[cfg(target_os = "linux")]
fn seq_read(path: &Path, file_size: u64) -> std::io::Result<(f64, bool)> {
    use linux_io::*;
    try_drop_caches();
    let mut f = open_direct_read(path)?;
    let mut buf = AlignedBuf::new(CHUNK, 4096)?;
    let slice = buf.as_mut_slice();
    let start = Instant::now();
    let mut total: u64 = 0;
    while total < file_size {
        match f.read(slice)? {
            0 => break,
            n => total += n as u64,
        }
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    Ok(((total as f64 / 1.0e6) / secs, false))
}

#[cfg(not(target_os = "linux"))]
fn seq_read(path: &Path, file_size: u64) -> std::io::Result<(f64, bool)> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; CHUNK];
    let start = Instant::now();
    let mut total: u64 = 0;
    while total < file_size {
        match f.read(&mut buf)? {
            0 => break,
            n => total += n as u64,
        }
    }
    let secs = start.elapsed().as_secs_f64().max(1e-9);
    Ok(((total as f64 / 1.0e6) / secs, true))
}

// ============================================================
// Random 4 KiB read latency
// ============================================================

#[cfg(target_os = "linux")]
fn rand_read(path: &Path, file_size: u64) -> std::io::Result<(f64, f64)> {
    use linux_io::*;
    let mut f = open_direct_read(path)?;
    let mut buf = AlignedBuf::new(RAND_BLOCK, 4096)?;
    let slice = buf.as_mut_slice();
    let max_blocks = (file_size / RAND_BLOCK as u64).max(1);
    let mut seed: u64 = 0xCAFEBABEDEADBEEF;
    let mut samples = Vec::with_capacity(RAND_OPS);
    for _ in 0..RAND_OPS {
        let off = (xorshift64(&mut seed) % max_blocks) * RAND_BLOCK as u64;
        f.seek(SeekFrom::Start(off))?;
        let t0 = Instant::now();
        f.read_exact(slice)?;
        let us = t0.elapsed().as_secs_f64() * 1.0e6;
        samples.push(us);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok((stats::percentile(&samples, 50.0), stats::percentile(&samples, 99.0)))
}

#[cfg(not(target_os = "linux"))]
fn rand_read(path: &Path, file_size: u64) -> std::io::Result<(f64, f64)> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; RAND_BLOCK];
    let max_blocks = (file_size / RAND_BLOCK as u64).max(1);
    let mut seed: u64 = 0xCAFEBABEDEADBEEF;
    let mut samples = Vec::with_capacity(RAND_OPS);
    for _ in 0..RAND_OPS {
        let off = (xorshift64(&mut seed) % max_blocks) * RAND_BLOCK as u64;
        f.seek(SeekFrom::Start(off))?;
        let t0 = Instant::now();
        f.read_exact(&mut buf)?;
        let us = t0.elapsed().as_secs_f64() * 1.0e6;
        samples.push(us);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok((stats::percentile(&samples, 50.0), stats::percentile(&samples, 99.0)))
}

// ============================================================
// Random 4 KiB write latency
// ============================================================

#[cfg(target_os = "linux")]
fn rand_write(path: &Path, file_size: u64) -> std::io::Result<(f64, f64)> {
    use linux_io::*;
    let mut f = open_direct_rw(path)?;
    let mut buf = AlignedBuf::new(RAND_BLOCK, 4096)?;
    let slice = buf.as_mut_slice();
    let max_blocks = (file_size / RAND_BLOCK as u64).max(1);
    let mut seed: u64 = 0xBADC0FFEE0DDF00D;
    let mut samples = Vec::with_capacity(RAND_OPS);
    for _ in 0..RAND_OPS {
        let off = (xorshift64(&mut seed) % max_blocks) * RAND_BLOCK as u64;
        f.seek(SeekFrom::Start(off))?;
        let t0 = Instant::now();
        f.write_all(slice)?;
        let us = t0.elapsed().as_secs_f64() * 1.0e6;
        samples.push(us);
    }
    f.sync_data()?;
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok((stats::percentile(&samples, 50.0), stats::percentile(&samples, 99.0)))
}

#[cfg(not(target_os = "linux"))]
fn rand_write(path: &Path, file_size: u64) -> std::io::Result<(f64, f64)> {
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let buf = vec![0xA5u8; RAND_BLOCK];
    let max_blocks = (file_size / RAND_BLOCK as u64).max(1);
    let mut seed: u64 = 0xBADC0FFEE0DDF00D;
    let mut samples = Vec::with_capacity(RAND_OPS);
    for _ in 0..RAND_OPS {
        let off = (xorshift64(&mut seed) % max_blocks) * RAND_BLOCK as u64;
        f.seek(SeekFrom::Start(off))?;
        let t0 = Instant::now();
        f.write_all(&buf)?;
        let us = t0.elapsed().as_secs_f64() * 1.0e6;
        samples.push(us);
    }
    f.sync_data()?;
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok((stats::percentile(&samples, 50.0), stats::percentile(&samples, 99.0)))
}

// ============================================================
// Public runner
// ============================================================

pub fn run(ram_mib: u64) -> std::io::Result<DiskResults> {
    let file_size = choose_file_size(ram_mib);
    let file_size_mib = file_size / (1024 * 1024);

    let mut path: PathBuf = std::env::temp_dir();
    path.push("sysbench_scratch.bin");
    let _guard = TempPath(path.clone());

    let seq_write_mbs = seq_write(&path, file_size)?;
    let (seq_read_mbs, seq_read_cached) = seq_read(&path, file_size)?;
    let (rand_read_p50_us, rand_read_p99_us) = rand_read(&path, file_size)?;
    let (rand_write_p50_us, rand_write_p99_us) = rand_write(&path, file_size)?;

    Ok(DiskResults {
        file_size_mib,
        seq_write_mbs,
        seq_read_mbs,
        seq_read_cached,
        rand_read_p50_us,
        rand_read_p99_us,
        rand_write_p50_us,
        rand_write_p99_us,
    })
}
