//! Responsibilities: measure this process and the local generated fixture only.
//! Not responsible for: workload execution or target hardware extrapolation.
//! Failure modes: lifetime maximum mislabeled as current memory, missed files during GC.
//! Used by the ignored encrypted owner benchmark.
use std::{path::Path, process::Command};
pub fn disk(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => disk(&entry.path()),
                Ok(meta) => meta.len(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
                Err(e) => panic!("{e}"),
            }
        })
        .sum()
}
pub fn uptime() -> String {
    String::from_utf8(Command::new("uptime").output().unwrap().stdout).unwrap().trim().to_owned()
}
#[cfg(target_os = "macos")]
pub fn rss() -> f64 {
    let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::uninit();
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    // Ask only about this process. task_info fills the correctly sized C structure
    // on success and never retains either output pointer.
    #[allow(deprecated)]
    let status = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    assert_eq!(status, libc::KERN_SUCCESS);
    unsafe { info.assume_init() }.resident_size as f64 / 1048576.
}
#[cfg(not(target_os = "macos"))]
pub fn rss() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/statm").unwrap();
    let pages: f64 = stat.split_whitespace().nth(1).unwrap().parse().unwrap();
    // sysconf returns the page size and has no memory-safety preconditions.
    pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as f64 / 1048576.
}
fn usage() -> libc::rusage {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // getrusage initializes the output on success and never retains its pointer.
    assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) }, 0);
    unsafe { usage.assume_init() }
}
pub fn peak() -> f64 {
    let bytes = usage().ru_maxrss as f64;
    if cfg!(target_os = "macos") { bytes / 1048576. } else { bytes / 1024. }
}
/// User and system time this process has used so far.
pub fn cpu_seconds() -> f64 {
    let usage = usage();
    let seconds = |time: libc::timeval| time.tv_sec as f64 + time.tv_usec as f64 / 1_000_000.;
    seconds(usage.ru_utime) + seconds(usage.ru_stime)
}
/// What this process has read from storage so far, past the page cache. Linux only: the runs
/// sized like the target machine happen there.
pub fn read_mib() -> Option<f64> {
    let io = std::fs::read_to_string("/proc/self/io").ok()?;
    let bytes = io.lines().find_map(|line| line.strip_prefix("read_bytes: "))?;
    Some(bytes.parse::<f64>().ok()? / 1048576.)
}
