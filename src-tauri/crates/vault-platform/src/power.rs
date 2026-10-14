//! Responsibilities: tell when this computer last woke from sleep or started, whichever is
//! later, so a schedule check can tell a scheduler that stopped from one that has not caught up
//! after a night asleep (`vault_core::overdue_issue`).
//! Not responsible for: deciding what counts as caught up, or watching for sleep and wake.
//! Failure modes: a computer that never slept since it started has no wake time (macOS reports
//! zero, Windows a wake time of zero); the time cannot be read, which must not be taken for a
//! wake a moment ago; Linux has none to give, and reports no installed schedule to check.

use chrono::{DateTime, Utc};

/// When this computer last woke or started, whichever is later; `None` where it can't be read.
#[cfg(target_os = "macos")]
pub fn awake_since() -> Option<DateTime<Utc>> {
    // `kern.waketime` is zero until the first sleep; `kern.boottime` is always set.
    [sysctl_time(c"kern.boottime"), sysctl_time(c"kern.waketime")].into_iter().flatten().max()
}

#[cfg(target_os = "macos")]
fn sysctl_time(name: &std::ffi::CStr) -> Option<DateTime<Utc>> {
    let mut value = libc::timeval { tv_sec: 0, tv_usec: 0 };
    let mut size = std::mem::size_of::<libc::timeval>();
    // SAFETY: `value` is a writable timeval and `size` its size, which is what these sysctls
    // return; nothing is written.
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut value).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || size != std::mem::size_of::<libc::timeval>() || value.tv_sec <= 0 {
        return None;
    }
    DateTime::from_timestamp(value.tv_sec, u32::try_from(value.tv_usec).ok()? * 1_000)
}

/// When this computer last woke or started, whichever is later; `None` where it can't be read.
#[cfg(target_os = "windows")]
pub fn awake_since() -> Option<DateTime<Utc>> {
    use windows_sys::Win32::System::{
        Power::{CallNtPowerInformation, LastWakeTime},
        WindowsProgramming::QueryInterruptTime,
    };
    // Interrupt time counts 100 ns ticks since the computer started, asleep or not.
    let mut now_ticks = 0_u64;
    // SAFETY: `now_ticks` is a writable u64.
    unsafe { QueryInterruptTime(&mut now_ticks) };
    // The interrupt time at the last wake; zero if it has not slept since it started.
    let mut wake_ticks = 0_u64;
    // SAFETY: the output buffer is a writable u64 of the size given, as `LastWakeTime` returns.
    let status = unsafe {
        CallNtPowerInformation(
            LastWakeTime,
            std::ptr::null(),
            0,
            (&raw mut wake_ticks).cast(),
            std::mem::size_of::<u64>() as u32,
        )
    };
    if status != 0 || wake_ticks > now_ticks {
        return None;
    }
    let awake_for = i64::try_from((now_ticks - wake_ticks) / 10).ok()?;
    Some(Utc::now() - chrono::Duration::microseconds(awake_for))
}

/// Linux reports no installed schedule (`scheduler::linux`), so nothing asks.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn awake_since() -> Option<DateTime<Utc>> {
    None
}
