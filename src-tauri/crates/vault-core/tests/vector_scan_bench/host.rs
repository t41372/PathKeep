//! Responsibilities: what the scan bench measures about its own process and disk: live
//! allocation bytes, physical footprint and free space before writing large files.
//! Not responsible for: timing, recall or anything about the candidates themselves.

use anyhow::{Result, ensure};
use serde::Serialize;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering as AtomicOrdering};

struct CountingAllocator;
static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);
static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
// SAFETY: allocation/deallocation are delegated unchanged to System. The atomic tracks requested
// allocation sizes only; it never touches the allocated memory or changes allocation semantics.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && COUNT_ALLOCATIONS.load(AtomicOrdering::Relaxed) {
            LIVE_BYTES.fetch_add(layout.size() as isize, AtomicOrdering::Relaxed);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if COUNT_ALLOCATIONS.load(AtomicOrdering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size() as isize, AtomicOrdering::Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() && COUNT_ALLOCATIONS.load(AtomicOrdering::Relaxed) {
            LIVE_BYTES.fetch_add(size as isize - layout.size() as isize, AtomicOrdering::Relaxed);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

pub(crate) fn begin_allocation_count() {
    LIVE_BYTES.store(0, AtomicOrdering::Relaxed);
    COUNT_ALLOCATIONS.store(true, AtomicOrdering::Relaxed);
}
pub(crate) fn end_allocation_count() -> usize {
    COUNT_ALLOCATIONS.store(false, AtomicOrdering::Relaxed);
    LIVE_BYTES.load(AtomicOrdering::Relaxed).max(0) as usize
}

#[derive(Clone, Copy, Serialize)]
pub(crate) struct Footprint {
    pub(crate) current: u64,
    pub(crate) peak: u64,
    pub(crate) method: &'static str,
}
pub(crate) fn footprint() -> Footprint {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: proc_pid_rusage writes a v4 struct into an appropriately sized/aligned buffer.
        let mut usage = unsafe { std::mem::zeroed::<libc::rusage_info_v4>() };
        let status = unsafe {
            libc::proc_pid_rusage(
                libc::getpid(),
                libc::RUSAGE_INFO_V4,
                (&mut usage as *mut libc::rusage_info_v4).cast(),
            )
        };
        assert_eq!(status, 0, "proc_pid_rusage failed");
        Footprint {
            current: usage.ri_phys_footprint,
            peak: usage.ri_lifetime_max_phys_footprint,
            method: "proc_pid_rusage v4 physical footprint (process lifetime peak)",
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
        assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) }, 0);
        let peak = usage.ru_maxrss as u64 * 1024;
        Footprint { current: peak, peak, method: "getrusage process max RSS, not current RSS" }
    }
    #[cfg(not(unix))]
    {
        Footprint { current: 0, peak: 0, method: "unavailable on this host" }
    }
}

pub(crate) fn disk_fits(path: &Path, bytes: u64) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let mut stat = unsafe { std::mem::zeroed::<libc::statvfs>() };
        // SAFETY: path is NUL-terminated; stat is a valid output buffer.
        ensure!(unsafe { libc::statvfs(path.as_ptr(), &mut stat) } == 0, "statvfs failed");
        let available = u128::from(stat.f_bavail) * u128::from(stat.f_frsize);
        Ok(available.saturating_sub(u128::from(bytes)) >= 10_000_000_000)
    }
    #[cfg(not(unix))]
    anyhow::bail!(
        "disk safety check is implemented for Unix hosts only: {} / {bytes}",
        path.display()
    )
}
