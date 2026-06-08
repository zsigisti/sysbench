// CPU affinity helpers.
//
// WHY THIS EXISTS — a subtle, severe benchmark bug:
//   The single-threaded suites pin the *current* (main) thread to one core to
//   cut scheduler migration noise. On Linux a child thread INHERITS its parent's
//   CPU affinity mask. So once the ST suites had pinned main to core 0, every
//   multi-threaded worker spawned afterwards inherited a "core 0 only" mask and
//   all of them piled onto a single core — collapsing MT throughput to ~1x of
//   ST regardless of core count.
//
// The fix: pin only for the duration of an ST run via `PinGuard`, which restores
// the full CPU set on drop, and explicitly reset main to all cores before MT.

#[cfg(target_os = "linux")]
mod imp {
    /// Number of logical CPUs as seen by the scheduler.
    fn cpu_count() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    /// Build a cpu_set_t covering cores `[0, n)`.
    unsafe fn full_set(n: usize) -> libc::cpu_set_t {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        for c in 0..n {
            libc::CPU_SET(c, &mut set);
        }
        set
    }

    /// Pin the calling thread to a single core. Returns true on success.
    pub fn pin_current(core: usize) -> bool {
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut set);
            libc::CPU_SET(core, &mut set);
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
        }
    }

    /// Reset the calling thread's affinity to every logical CPU.
    pub fn reset_current() {
        let n = cpu_count();
        unsafe {
            let set = full_set(n);
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    // On non-Linux we lean on core_affinity for best-effort pinning and treat
    // "reset" as a no-op (the inheritance pitfall is Linux-specific).
    pub fn pin_current(core: usize) -> bool {
        if let Some(ids) = core_affinity::get_core_ids() {
            if let Some(&id) = ids.get(core) {
                return core_affinity::set_for_current(id);
            }
        }
        false
    }
    pub fn reset_current() {}
}

/// Pin the current thread to `core` for the lifetime of the guard; the full CPU
/// set is restored on drop so the pin never leaks into later (MT) work.
pub struct PinGuard;

impl PinGuard {
    pub fn pin(core: usize) -> Self {
        let _ = imp::pin_current(core);
        PinGuard
    }
}

impl Drop for PinGuard {
    fn drop(&mut self) {
        imp::reset_current();
    }
}

/// Reset the current thread to all cores. Call on the main thread before
/// spawning MT workers so they inherit an unrestricted mask.
pub fn reset_to_all_cores() {
    imp::reset_current();
}

/// Best-effort pin used by spawned workers (does not need restoring — the worker
/// exits right after its run).
pub fn pin_worker(core: usize) {
    let _ = imp::pin_current(core);
}
