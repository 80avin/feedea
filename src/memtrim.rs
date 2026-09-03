use std::time::Duration;

pub const DEFAULT_TRIM_MINUTES: u64 = 5;
pub const DEFAULT_ARENA_MAX: i32 = 2;

// glibc keeps pages freed by the app in per-thread malloc arenas instead of
// returning them to the OS, so RSS ratchets up to the workload high-water mark
// and never comes back down, even when the server is idle. There is no
// live-object leak: the process simply never hands the pages back.
//
// Two levers fix it:
//  1. mallopt(M_ARENA_MAX, n) bounds how many arenas glibc is allowed to
//     create. Fewer arenas means freed memory is reused across threads instead
//     of being stranded in per-thread arenas, which keeps RSS flat under load.
//  2. malloc_trim(0) releases the (small) leftover free pages back to the OS
//     on a slow timer.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
    fn mallopt(param: i32, value: i32) -> i32;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
const M_ARENA_MAX: i32 = -8;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn set_arena_max(max: i32) {
    unsafe {
        mallopt(M_ARENA_MAX, max);
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn set_arena_max(_max: i32) {}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim_memory() {}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim_memory() {
    unsafe {
        malloc_trim(0);
    }
}

pub async fn trim_loop(interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;
        // malloc_trim locks the heap briefly; keep it off the async worker.
        let _ = tokio::task::spawn_blocking(trim_memory).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_memory_runs_without_panicking() {
        trim_memory();
        set_arena_max(DEFAULT_ARENA_MAX);
    }

    #[test]
    fn default_interval_is_five_minutes() {
        assert_eq!(
            Duration::from_secs(DEFAULT_TRIM_MINUTES * 60),
            Duration::from_secs(5 * 60)
        );
    }
}
