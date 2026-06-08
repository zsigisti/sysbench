// `crux info` — a thorough system report (fastfetch, but deeper).
//
// Reads everything locally from /proc, /sys and a few libc syscalls. No network,
// no heavy dependencies. Renders an aligned, colourised key/value table.

mod collect;
mod format;

use format::{bar, human_bytes, Style};

/// Render the full system report to stdout.
pub fn run() {
    let st = Style::new();
    let mut rows: Vec<(String, String)> = Vec::new();

    let user = collect::username();
    let host = collect::hostname();
    let header = format!("{}@{}", st.accent(&user), st.accent(&host));
    let rule = "─".repeat(user.len() + host.len() + 1);

    let cpu = collect::cpu_info();
    let mem_total = format::meminfo_kib("MemTotal:").unwrap_or(0) * 1024;
    let mem_avail = format::meminfo_kib("MemAvailable:").unwrap_or(0) * 1024;
    let mem_used = mem_total.saturating_sub(mem_avail);
    let swap_total = format::meminfo_kib("SwapTotal:").unwrap_or(0) * 1024;
    let swap_free = format::meminfo_kib("SwapFree:").unwrap_or(0) * 1024;
    let swap_used = swap_total.saturating_sub(swap_free);

    rows.push(("OS".into(), collect::os_pretty()));
    rows.push(("Host".into(), host.clone()));
    rows.push(("Kernel".into(), format!("{} ({})", collect::kernel(), collect::arch())));
    rows.push(("Uptime".into(), collect::uptime()));
    rows.push(("Packages".into(), collect::packages()));
    rows.push(("Shell".into(), collect::shell()));

    let freq = match (cpu.cur_mhz, cpu.max_mhz) {
        (Some(c), Some(m)) => format!(" @ {:.0}/{:.0} MHz", c, m),
        (Some(c), None) => format!(" @ {:.0} MHz", c),
        (None, Some(m)) => format!(" @ max {:.0} MHz", m),
        (None, None) => String::new(),
    };
    rows.push((
        "CPU".into(),
        format!("{} ({}C/{}T){}", cpu.model, cpu.physical, cpu.logical, freq),
    ));
    rows.push(("Cache".into(), cpu.cache));
    rows.push(("Load".into(), collect::load_avg()));

    for g in collect::gpus() {
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

    for (mount, fstype, total, used) in collect::disks() {
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

    for (name, c) in &collect::temperatures() {
        rows.push((format!("Temp ({})", name), format!("{:.1} °C", c)));
    }

    if let Some(bat) = collect::battery() {
        rows.push(("Battery".into(), bat));
    }

    for (name, ips, mac) in collect::network() {
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
        st.title("CRUCIBLE"),
        st.paint("2", "— crux info · deep system report")
    );
    println!();
    for (k, v) in &rows {
        println!("  {:<width$}  {}", st.key(k), v, width = key_w);
    }
    println!();
}
