//! Responsibilities: sample resident memory in the isolated benchmark process.
//! Not responsible for: fixture generation or claiming a kernel high-water mark.
//! Failure modes: gVisor omits VmHWM, page size is assumed, or setup pollutes phase peaks.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

pub struct PeakRss {
    peak: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl PeakRss {
    pub fn start() -> Self {
        let peak = Arc::new(AtomicU64::new(current()));
        let stop = Arc::new(AtomicBool::new(false));
        let sampled = peak.clone();
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                sampled.fetch_max(current(), Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        Self { peak, stop, thread: Some(thread) }
    }
    pub fn peak(&self) -> u64 {
        self.peak.fetch_max(current(), Ordering::Relaxed);
        self.peak.load(Ordering::Relaxed)
    }
    pub fn reset(&self) {
        self.peak.store(current(), Ordering::Relaxed);
    }
}
impl Drop for PeakRss {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn current() -> u64 {
    let text = std::fs::read_to_string("/proc/self/statm").unwrap();
    let pages: u64 = text.split_whitespace().nth(1).unwrap().parse().unwrap();
    // sysconf has no memory-safety preconditions and reports this Sandbox's page size.
    pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64 / 1024
}

/// Whole-child high-water RSS is provided by the Linux VM kernel. Phase-local
/// peaks still use the sampler because the kernel's high-water mark cannot reset.
pub fn kernel_peak() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .expect("the benchmark requires Linux VM VmHWM")
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
