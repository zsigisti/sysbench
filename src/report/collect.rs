// System fact collectors for `crux info`. Everything comes from /proc, /sys and
// a couple of libc syscalls — no external services, Linux-focused.

use std::fs;
use std::path::Path;

use super::format::{first_line, read, run_cmd};

pub fn hostname() -> String {
    first_line("/proc/sys/kernel/hostname")
        .filter(|s| !s.is_empty())
        .or_else(|| first_line("/etc/hostname"))
        .unwrap_or_else(|| "unknown".into())
}

pub fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".into())
}

pub fn os_pretty() -> String {
    if let Some(s) = read("/etc/os-release") {
        for line in s.lines() {
            if let Some(r) = line.strip_prefix("PRETTY_NAME=") {
                return r.trim_matches('"').to_string();
            }
        }
    }
    "unknown".into()
}

pub fn arch() -> String {
    run_cmd("uname", &["-m"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| std::env::consts::ARCH.to_string())
}

pub fn kernel() -> String {
    first_line("/proc/sys/kernel/osrelease").unwrap_or_else(|| "unknown".into())
}

pub fn uptime() -> String {
    let secs = read("/proc/uptime")
        .and_then(|s| s.split_whitespace().next().map(|v| v.to_string()))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let mut parts = Vec::new();
    if d > 0 {
        parts.push(format!("{}d", d));
    }
    if h > 0 {
        parts.push(format!("{}h", h));
    }
    parts.push(format!("{}m", m));
    parts.join(" ")
}

pub fn shell() -> String {
    std::env::var("SHELL")
        .ok()
        .map(|s| {
            Path::new(&s)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or(s)
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Best-effort package counts across common managers.
pub fn packages() -> String {
    let mut out = Vec::new();
    if let Some(s) = read("/var/lib/dpkg/status") {
        let n = s.matches("\nStatus: install ok installed").count();
        if n > 0 {
            out.push(format!("{} (dpkg)", n));
        }
    }
    if let Ok(rd) = fs::read_dir("/var/lib/pacman/local") {
        let n = rd
            .filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false))
            .count();
        if n > 1 {
            out.push(format!("{} (pacman)", n - 1));
        }
    }
    if let Some(s) = read("/lib/apk/db/installed") {
        let n = s.matches("\nP:").count();
        if n > 0 {
            out.push(format!("{} (apk)", n));
        }
    }
    if Path::new("/var/lib/rpm").exists() {
        if let Some(s) = run_cmd("rpm", &["-qa"]) {
            let n = s.lines().filter(|l| !l.trim().is_empty()).count();
            if n > 0 {
                out.push(format!("{} (rpm)", n));
            }
        }
    }
    if let Some(s) = run_cmd("flatpak", &["list", "--app"]) {
        let n = s.lines().filter(|l| !l.trim().is_empty()).count();
        if n > 0 {
            out.push(format!("{} (flatpak)", n));
        }
    }
    if out.is_empty() {
        "unknown".into()
    } else {
        out.join(", ")
    }
}

pub struct Cpu {
    pub model: String,
    pub logical: usize,
    pub physical: usize,
    pub cur_mhz: Option<f64>,
    pub max_mhz: Option<f64>,
    pub cache: String,
}

pub fn cpu_info() -> Cpu {
    let info = read("/proc/cpuinfo").unwrap_or_default();
    let mut model = String::from("unknown");
    let mut logical = 0usize;
    let mut mhz_sum = 0.0f64;
    let mut mhz_n = 0usize;
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut cur_phys = String::new();

    for line in info.lines() {
        if let Some(r) = line.split_once(':') {
            let k = r.0.trim();
            let v = r.1.trim();
            match k {
                "model name" => {
                    if model == "unknown" {
                        model = v.to_string();
                    }
                    logical += 1;
                }
                "Hardware" | "Model" if model == "unknown" => model = v.to_string(),
                "cpu MHz" => {
                    if let Ok(f) = v.parse::<f64>() {
                        mhz_sum += f;
                        mhz_n += 1;
                    }
                }
                "physical id" => cur_phys = v.to_string(),
                "core id" => pairs.push((cur_phys.clone(), v.to_string())),
                _ => {}
            }
        }
    }
    if logical == 0 {
        logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    }
    let physical = {
        let mut uniq = pairs.clone();
        uniq.sort();
        uniq.dedup();
        if uniq.is_empty() {
            logical
        } else {
            uniq.len()
        }
    };

    let cur_mhz = if mhz_n > 0 {
        Some(mhz_sum / mhz_n as f64)
    } else {
        first_line("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|k| k / 1000.0)
    };
    let max_mhz = first_line("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .or_else(|| first_line("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq"))
        .and_then(|s| s.parse::<f64>().ok())
        .map(|k| k / 1000.0);

    Cpu {
        model,
        logical,
        physical,
        cur_mhz,
        max_mhz,
        cache: cpu_cache(),
    }
}

fn cpu_cache() -> String {
    let mut parts = Vec::new();
    for idx in 0..8 {
        let base = format!("/sys/devices/system/cpu/cpu0/cache/index{}", idx);
        if !Path::new(&base).exists() {
            break;
        }
        let level = first_line(&format!("{}/level", base)).unwrap_or_default();
        let ty = first_line(&format!("{}/type", base)).unwrap_or_default();
        let size = first_line(&format!("{}/size", base)).unwrap_or_default();
        let tag = match ty.as_str() {
            "Data" => format!("L{}d", level),
            "Instruction" => format!("L{}i", level),
            _ => format!("L{}", level),
        };
        parts.push(format!("{} {}", tag, size));
    }
    if parts.is_empty() {
        "unknown".into()
    } else {
        parts.join(", ")
    }
}

pub fn load_avg() -> String {
    read("/proc/loadavg")
        .map(|s| {
            let f: Vec<&str> = s.split_whitespace().take(3).collect();
            f.join(" ")
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn gpus() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(s) = run_cmd("lspci", &[]) {
        for line in s.lines() {
            if line.contains("VGA compatible controller")
                || line.contains("3D controller")
                || line.contains("Display controller")
            {
                // "00:02.0 VGA compatible controller: NVIDIA ... (rev a1)" — the
                // model is the text after the FIRST ": "; drop the trailing
                // "(rev xx)" revision suffix.
                if let Some(idx) = line.find(": ") {
                    let mut model = line[idx + 2..].trim().to_string();
                    if let Some(p) = model.rfind(" (rev ") {
                        model.truncate(p);
                    }
                    out.push(model);
                } else {
                    out.push(line.trim().to_string());
                }
            }
        }
    }
    if out.is_empty() {
        if let Ok(rd) = fs::read_dir("/sys/class/drm") {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("card") && !name.contains('-') {
                    let uevent =
                        read(&format!("/sys/class/drm/{}/device/uevent", name)).unwrap_or_default();
                    for l in uevent.lines() {
                        if let Some(d) = l.strip_prefix("DRIVER=") {
                            out.push(format!("{} ({})", name, d));
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn temperatures() -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir("/sys/class/thermal") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("thermal_zone") {
                let ty = first_line(&format!("/sys/class/thermal/{}/type", name))
                    .unwrap_or_else(|| name.clone());
                if let Some(t) = first_line(&format!("/sys/class/thermal/{}/temp", name))
                    .and_then(|s| s.parse::<f64>().ok())
                {
                    out.push((ty, t / 1000.0));
                }
            }
        }
    }
    out
}

pub fn battery() -> Option<String> {
    let rd = fs::read_dir("/sys/class/power_supply").ok()?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let base = format!("/sys/class/power_supply/{}", name);
        let ty = first_line(&format!("{}/type", base)).unwrap_or_default();
        if ty == "Battery" {
            let cap = first_line(&format!("{}/capacity", base)).unwrap_or_default();
            let status = first_line(&format!("{}/status", base)).unwrap_or_default();
            return Some(format!("{}% ({})", cap, status));
        }
    }
    None
}

// ---------- Disks (statvfs over /proc/mounts) ----------

#[cfg(unix)]
fn statvfs(path: &str) -> Option<(u64, u64)> {
    use std::ffi::CString;
    let c = CString::new(path).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let frsize = s.f_frsize as u64;
    let total = s.f_blocks as u64 * frsize;
    let avail = s.f_bavail as u64 * frsize;
    Some((total, total.saturating_sub(avail)))
}

#[cfg(not(unix))]
fn statvfs(_path: &str) -> Option<(u64, u64)> {
    None
}

/// (mount, fstype, total, used) for real (non-virtual) mounted filesystems.
pub fn disks() -> Vec<(String, String, u64, u64)> {
    let mut out = Vec::new();
    let real_fs = [
        "ext4", "ext3", "ext2", "xfs", "btrfs", "zfs", "f2fs", "vfat", "exfat", "ntfs", "ntfs3",
        "overlay", "fuseblk",
    ];
    if let Some(s) = read("/proc/mounts") {
        let mut seen = Vec::new();
        for line in s.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 3 {
                continue;
            }
            let (dev, mount, fstype) = (f[0], f[1], f[2]);
            if !real_fs.contains(&fstype) || !dev.starts_with('/') {
                continue;
            }
            if seen.contains(&mount.to_string()) {
                continue;
            }
            seen.push(mount.to_string());
            if let Some((total, used)) = statvfs(mount) {
                if total > 0 {
                    out.push((mount.to_string(), fstype.to_string(), total, used));
                }
            }
        }
    }
    out
}

// ---------- Network (getifaddrs) ----------

#[cfg(target_os = "linux")]
pub fn network() -> Vec<(String, Vec<String>, Option<String>)> {
    use super::format::format_ipv6;
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, (Vec<String>, Option<String>)> = BTreeMap::new();
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return Vec::new();
        }
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_name.is_null() {
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name)
                    .to_string_lossy()
                    .into_owned();
                if name != "lo" {
                    let entry = map.entry(name).or_default();
                    if !ifa.ifa_addr.is_null() {
                        let fam = (*ifa.ifa_addr).sa_family as i32;
                        if fam == libc::AF_INET {
                            let sa = ifa.ifa_addr as *const libc::sockaddr_in;
                            let b = (*sa).sin_addr.s_addr.to_ne_bytes();
                            entry.0.push(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]));
                        } else if fam == libc::AF_INET6 {
                            let sa = ifa.ifa_addr as *const libc::sockaddr_in6;
                            entry.0.push(format_ipv6(&(*sa).sin6_addr.s6_addr));
                        } else if fam == libc::AF_PACKET {
                            let sa = ifa.ifa_addr as *const libc::sockaddr_ll;
                            let sll = &*sa;
                            if sll.sll_halen as usize == 6 {
                                let a = sll.sll_addr;
                                entry.1 = Some(format!(
                                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                    a[0], a[1], a[2], a[3], a[4], a[5]
                                ));
                            }
                        }
                    }
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
    }
    map.into_iter().map(|(k, (ips, mac))| (k, ips, mac)).collect()
}

#[cfg(not(target_os = "linux"))]
pub fn network() -> Vec<(String, Vec<String>, Option<String>)> {
    Vec::new()
}
