// sysinfo — a thorough system information display (fastfetch, but deeper).
//
// Reads everything from /proc, /sys and a couple of libc syscalls. No external
// services, no heavy dependencies. Linux-focused.

use std::fs;
use std::path::Path;
use std::process::Command;

// ============================================================
// Colour / formatting
// ============================================================

struct Style {
    on: bool,
}

impl Style {
    fn new() -> Self {
        let on = std::env::var_os("NO_COLOR").is_none()
            && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true);
        Style { on }
    }
    fn paint(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{}m{}\x1b[0m", code, s)
        } else {
            s.to_string()
        }
    }
    fn title(&self, s: &str) -> String {
        self.paint("1;36", s) // bold cyan
    }
    fn key(&self, s: &str) -> String {
        self.paint("1;32", s) // bold green
    }
    fn accent(&self, s: &str) -> String {
        self.paint("1;35", s) // bold magenta
    }
}

fn human_bytes(b: u64) -> String {
    const U: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", b, U[i])
    } else {
        format!("{:.2} {}", v, U[i])
    }
}

fn bar(used: u64, total: u64, width: usize, st: &Style) -> String {
    if total == 0 {
        return String::new();
    }
    let frac = (used as f64 / total as f64).clamp(0.0, 1.0);
    let filled = (frac * width as f64).round() as usize;
    let pct = frac * 100.0;
    let colour = if pct > 90.0 {
        "1;31"
    } else if pct > 75.0 {
        "1;33"
    } else {
        "1;32"
    };
    let fill = st.paint(colour, &"━".repeat(filled));
    let empty = "─".repeat(width.saturating_sub(filled));
    format!("[{}{}] {:.0}%", fill, empty, pct)
}

// ============================================================
// Small file helpers
// ============================================================

fn read(p: &str) -> Option<String> {
    fs::read_to_string(p).ok()
}

fn first_line(p: &str) -> Option<String> {
    read(p).map(|s| s.lines().next().unwrap_or("").trim().to_string())
}

fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

fn meminfo_kib(key: &str) -> Option<u64> {
    let s = read("/proc/meminfo")?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            return rest.split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    None
}

// ============================================================
// Individual collectors
// ============================================================

fn hostname() -> String {
    first_line("/proc/sys/kernel/hostname")
        .filter(|s| !s.is_empty())
        .or_else(|| first_line("/etc/hostname"))
        .unwrap_or_else(|| "unknown".into())
}

fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".into())
}

fn os_pretty() -> String {
    if let Some(s) = read("/etc/os-release") {
        for line in s.lines() {
            if let Some(r) = line.strip_prefix("PRETTY_NAME=") {
                return r.trim_matches('"').to_string();
            }
        }
    }
    "unknown".into()
}

fn arch() -> String {
    run_cmd("uname", &["-m"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| std::env::consts::ARCH.to_string())
}

fn kernel() -> String {
    first_line("/proc/sys/kernel/osrelease").unwrap_or_else(|| "unknown".into())
}

fn uptime() -> String {
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

fn shell() -> String {
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

fn packages() -> String {
    let mut out = Vec::new();
    // dpkg (Debian/Ubuntu)
    if let Some(s) = read("/var/lib/dpkg/status") {
        let n = s.matches("\nStatus: install ok installed").count();
        if n > 0 {
            out.push(format!("{} (dpkg)", n));
        }
    }
    // pacman (Arch)
    if let Ok(rd) = fs::read_dir("/var/lib/pacman/local") {
        let n = rd.filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false)).count();
        if n > 1 {
            out.push(format!("{} (pacman)", n - 1));
        }
    }
    // apk (Alpine)
    if let Some(s) = read("/lib/apk/db/installed") {
        let n = s.matches("\nP:").count();
        if n > 0 {
            out.push(format!("{} (apk)", n));
        }
    }
    // rpm (Fedora/RHEL/SUSE) — only if the DB dir exists, via the rpm tool
    if Path::new("/var/lib/rpm").exists() {
        if let Some(s) = run_cmd("rpm", &["-qa"]) {
            let n = s.lines().filter(|l| !l.trim().is_empty()).count();
            if n > 0 {
                out.push(format!("{} (rpm)", n));
            }
        }
    }
    // flatpak
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

struct Cpu {
    model: String,
    logical: usize,
    physical: usize,
    cur_mhz: Option<f64>,
    max_mhz: Option<f64>,
    cache: String,
}

fn cpu_info() -> Cpu {
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

fn load_avg() -> String {
    read("/proc/loadavg")
        .map(|s| {
            let f: Vec<&str> = s.split_whitespace().take(3).collect();
            f.join(" ")
        })
        .unwrap_or_else(|| "unknown".into())
}

fn gpus() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(s) = run_cmd("lspci", &[]) {
        for line in s.lines() {
            if line.contains("VGA compatible controller")
                || line.contains("3D controller")
                || line.contains("Display controller")
            {
                // Format: "00:02.0 VGA compatible controller: Intel ... "
                // The model is the text after the SECOND ": ".
                if let Some(idx) = line.find(": ") {
                    if let Some(idx2) = line[idx + 2..].find(": ") {
                        out.push(line[idx + 2 + idx2 + 2..].trim().to_string());
                        continue;
                    }
                }
                out.push(line.trim().to_string());
            }
        }
    }
    if out.is_empty() {
        // Fall back to DRM driver names from sysfs
        if let Ok(rd) = fs::read_dir("/sys/class/drm") {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("card") && !name.contains('-') {
                    let uevent = read(&format!("/sys/class/drm/{}/device/uevent", name))
                        .unwrap_or_default();
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

fn temperatures() -> Vec<(String, f64)> {
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

fn battery() -> Option<String> {
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

fn disks() -> Vec<(String, String, u64, u64)> {
    // (mount, fstype, total, used)
    let mut out = Vec::new();
    let real_fs = [
        "ext4", "ext3", "ext2", "xfs", "btrfs", "zfs", "f2fs", "vfat", "exfat",
        "ntfs", "ntfs3", "overlay", "fuseblk",
    ];
    if let Some(s) = read("/proc/mounts") {
        let mut seen = Vec::new();
        for line in s.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 3 {
                continue;
            }
            let (dev, mount, fstype) = (f[0], f[1], f[2]);
            if !real_fs.contains(&fstype) {
                continue;
            }
            if !dev.starts_with('/') {
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
fn network() -> Vec<(String, Vec<String>, Option<String>)> {
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
                            let hl = sll.sll_halen as usize;
                            if hl == 6 {
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
fn network() -> Vec<(String, Vec<String>, Option<String>)> {
    Vec::new()
}

fn format_ipv6(b: &[u8; 16]) -> String {
    let groups: [u16; 8] = std::array::from_fn(|i| ((b[2 * i] as u16) << 8) | b[2 * i + 1] as u16);
    // find the longest run of zero groups for :: compression
    let (mut best_start, mut best_len) = (0usize, 0usize);
    let (mut cur_start, mut cur_len) = (0usize, 0usize);
    for (i, &g) in groups.iter().enumerate() {
        if g == 0 {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_len = cur_len;
                best_start = cur_start;
            }
        } else {
            cur_len = 0;
        }
    }
    if best_len < 2 {
        return groups
            .iter()
            .map(|g| format!("{:x}", g))
            .collect::<Vec<_>>()
            .join(":");
    }
    let mut out = String::new();
    let mut i = 0;
    while i < 8 {
        if i == best_start {
            out.push_str("::");
            i += best_len;
            continue;
        }
        if !out.is_empty() && !out.ends_with(':') {
            out.push(':');
        }
        out.push_str(&format!("{:x}", groups[i]));
        i += 1;
    }
    out
}

// ============================================================
// Main
// ============================================================

fn main() {
    let st = Style::new();
    let mut rows: Vec<(String, String)> = Vec::new();

    let user = username();
    let host = hostname();
    let header = format!("{}@{}", st.accent(&user), st.accent(&host));
    let rule = "─".repeat(user.len() + host.len() + 1);

    let cpu = cpu_info();
    let mem_total = meminfo_kib("MemTotal:").unwrap_or(0) * 1024;
    let mem_avail = meminfo_kib("MemAvailable:").unwrap_or(0) * 1024;
    let mem_used = mem_total.saturating_sub(mem_avail);
    let swap_total = meminfo_kib("SwapTotal:").unwrap_or(0) * 1024;
    let swap_free = meminfo_kib("SwapFree:").unwrap_or(0) * 1024;
    let swap_used = swap_total.saturating_sub(swap_free);

    rows.push(("OS".into(), os_pretty()));
    rows.push(("Host".into(), host.clone()));
    rows.push(("Kernel".into(), format!("{} ({})", kernel(), arch())));
    rows.push(("Uptime".into(), uptime()));
    rows.push(("Packages".into(), packages()));
    rows.push(("Shell".into(), shell()));

    let freq = match (cpu.cur_mhz, cpu.max_mhz) {
        (Some(c), Some(m)) => format!(" @ {:.0}/{:.0} MHz", c, m),
        (Some(c), None) => format!(" @ {:.0} MHz", c),
        (None, Some(m)) => format!(" @ max {:.0} MHz", m),
        (None, None) => String::new(),
    };
    rows.push((
        "CPU".into(),
        format!(
            "{} ({}C/{}T){}",
            cpu.model, cpu.physical, cpu.logical, freq
        ),
    ));
    rows.push(("Cache".into(), cpu.cache));
    rows.push(("Load".into(), load_avg()));

    for g in gpus() {
        rows.push(("GPU".into(), g));
    }

    rows.push((
        "Memory".into(),
        format!(
            "{} / {}  {}",
            human_bytes(mem_used),
            human_bytes(mem_total),
            bar(mem_used, mem_total, 20, &st)
        ),
    ));
    if swap_total > 0 {
        rows.push((
            "Swap".into(),
            format!(
                "{} / {}  {}",
                human_bytes(swap_used),
                human_bytes(swap_total),
                bar(swap_used, swap_total, 20, &st)
            ),
        ));
    }

    for (mount, fstype, total, used) in disks() {
        rows.push((
            format!("Disk ({})", mount),
            format!(
                "{} / {} [{}]  {}",
                human_bytes(used),
                human_bytes(total),
                fstype,
                bar(used, total, 20, &st)
            ),
        ));
    }

    let temps = temperatures();
    for (name, c) in &temps {
        rows.push((format!("Temp ({})", name), format!("{:.1} °C", c)));
    }

    if let Some(bat) = battery() {
        rows.push(("Battery".into(), bat));
    }

    for (name, ips, mac) in network() {
        let mut detail = Vec::new();
        if let Some(m) = mac {
            detail.push(format!("MAC {}", m));
        }
        if !ips.is_empty() {
            detail.push(ips.join(", "));
        }
        if !detail.is_empty() {
            rows.push((format!("Net ({})", name), detail.join("  ")));
        }
    }

    // ---- render ----
    let key_w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    println!();
    println!("  {}", header);
    println!("  {}", rule);
    println!(
        "  {} {}",
        st.title("sysinfo"),
        st.paint("2", "— thorough system report")
    );
    println!();
    for (k, v) in &rows {
        println!("  {:<width$}  {}", st.key(k), v, width = key_w);
    }
    println!();
}
