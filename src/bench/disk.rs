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
    pub dir: String,
    pub on_tmpfs: bool,
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
const MIN_FILE: u64 = 256 * 1024 * 1024; // 256 MiB — below this the test is meaningless

fn choose_file_size(ram_mib: u64) -> u64 {
    let target = (ram_mib * 2).max(2048) * 1024 * 1024;
    target.min(4 * 1024 * 1024 * 1024)
}

/// Free bytes available to an unprivileged user on the filesystem holding `dir`.
#[cfg(unix)]
fn available_bytes(dir: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    Some(s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(not(unix))]
fn available_bytes(_dir: &Path) -> Option<u64> {
    None
}

/// True if `dir` lives on a RAM-backed filesystem (tmpfs/ramfs). O_DIRECT is a
/// no-op there, so the "disk" numbers would actually be memory speed — we detect
/// this and flag it loudly rather than reporting fake 16 GB/s / 1 µs results.
#[cfg(target_os = "linux")]
fn is_tmpfs(dir: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    const TMPFS_MAGIC: i64 = 0x0102_1994;
    const RAMFS_MAGIC: i64 = 0x8584_58f6u32 as i64;
    let Ok(c) = CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        return false;
    }
    let t = s.f_type as i64;
    t == TMPFS_MAGIC || t == RAMFS_MAGIC
}

#[cfg(not(target_os = "linux"))]
fn is_tmpfs(_dir: &Path) -> bool {
    false
}

/// Pick the scratch directory: an explicit `--dir`, else the current working
/// directory (almost always real disk), falling back to the temp dir. We avoid
/// silently defaulting to /tmp, which is tmpfs on most modern systemd distros.
fn choose_scratch_dir(requested: Option<PathBuf>) -> PathBuf {
    if let Some(d) = requested {
        return d;
    }
    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
}

/// Clamp the target file size so the benchmark never tries to fill the disk.
/// Uses at most half of the free space and leaves headroom; errors if the
/// filesystem can't host a meaningful test.
fn fit_to_space(dir: &Path, target: u64) -> std::io::Result<u64> {
    let mut size = target;
    if let Some(avail) = available_bytes(dir) {
        let usable = (avail / 2).min(avail.saturating_sub(avail / 10));
        size = size.min(usable);
    }
    size = (size / CHUNK as u64) * CHUNK as u64; // align to CHUNK
    if size < MIN_FILE {
        let avail_mib = available_bytes(dir).unwrap_or(0) / (1024 * 1024);
        return Err(std::io::Error::other(format!(
            "not enough free space in {} ({} MiB available, need ~512 MiB) — skipping disk test",
            dir.display(),
            avail_mib
        )));
    }
    Ok(size)
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

        #[allow(dead_code)]
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

pub fn run(ram_mib: u64, dir: Option<PathBuf>) -> std::io::Result<DiskResults> {
    let dir = choose_scratch_dir(dir);
    let on_tmpfs = is_tmpfs(&dir);
    let file_size = fit_to_space(&dir, choose_file_size(ram_mib))?;
    let file_size_mib = file_size / (1024 * 1024);

    let mut path: PathBuf = dir.clone();
    path.push(".crux_scratch.bin");
    let _guard = TempPath(path.clone());

    let seq_write_mbs = seq_write(&path, file_size)?;
    let (seq_read_mbs, seq_read_cached) = seq_read(&path, file_size)?;
    let (rand_read_p50_us, rand_read_p99_us) = rand_read(&path, file_size)?;
    let (rand_write_p50_us, rand_write_p99_us) = rand_write(&path, file_size)?;

    Ok(DiskResults {
        dir: dir.display().to_string(),
        on_tmpfs,
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
