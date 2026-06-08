# `crux info` — Deep System Report

`crux info` (or the `sysinfo` alias) prints a single aligned, colourised report.
It reads everything locally from `/proc`, `/sys`, and a few libc syscalls — **no
network, no external services**, and only `lspci` is shelled out to (with a
sysfs fallback). Colour is suppressed when `NO_COLOR` is set or `TERM=dumb`.

## What it reports

| Field | Source |
|-------|--------|
| **User@Host** | `$USER` / `/proc/sys/kernel/hostname` |
| **OS** | `PRETTY_NAME` from `/etc/os-release` |
| **Kernel** | `/proc/sys/kernel/osrelease` + `uname -m` |
| **Uptime** | `/proc/uptime`, formatted `d/h/m` |
| **Packages** | dpkg, pacman, apk, rpm, flatpak (best-effort, whichever exist) |
| **Shell** | basename of `$SHELL` |
| **CPU** | model, physical/logical core counts, current/max MHz | from `/proc/cpuinfo` + `cpufreq` |
| **Cache** | full L1d/L1i/L2/L3 hierarchy from `/sys/devices/system/cpu/cpu0/cache/index*` |
| **Load** | `/proc/loadavg` (1/5/15 min) |
| **GPU** | parsed from `lspci`; falls back to the DRM driver name in `/sys/class/drm/*/device/uevent` |
| **Memory / Swap** | `/proc/meminfo`, shown used/total with a coloured bar |
| **Disks** | every real (non-virtual) mount from `/proc/mounts`, sized via `statvfs`, with usage bars |
| **Thermals** | every `/sys/class/thermal/thermal_zone*` |
| **Battery** | `/sys/class/power_supply/*` of type `Battery` (capacity + status) |
| **Network** | per interface (via `getifaddrs`): MAC, IPv4, and IPv6 with `::` zero-run compression; `lo` excluded |

## How it differs from fastfetch

- **Full cache hierarchy**, not just a one-line CPU string.
- **Every** mounted real filesystem with a usage bar, not only `/`.
- **All** thermal zones and power supplies.
- **All** addresses per interface, including multiple IPv6 addresses, properly
  compressed.

## Bars

Usage bars are 20 cells wide and colour by fill: green `< 75%`, yellow `< 90%`,
red otherwise.

## Disk filtering

Only "real" filesystem types are shown: `ext{2,3,4}`, `xfs`, `btrfs`, `zfs`,
`f2fs`, `vfat`, `exfat`, `ntfs`/`ntfs3`, `overlay`, `fuseblk`. Pseudo/virtual
mounts (tmpfs, proc, sysfs, cgroup, …) and bind-duplicate mountpoints are
skipped. The device must be a real path (`/dev/...`).
