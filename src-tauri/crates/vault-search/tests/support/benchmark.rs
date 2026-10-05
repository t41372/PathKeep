//! Responsibilities: streaming realistic page projections and process measurements.
//! Not responsible for: indexing, scoring, or extrapolating to target hardware.
//! Failure modes: fixture growth hides memory use; correlated fields inflate selectivity;
//! high-water RSS is mislabeled steady RSS; merge GC removes a file while disk usage is sampled. The benchmark uses separate processes,
//! deterministic counts, actual resident bytes, and checks disk size during construction.
use std::path::Path;
use std::process::Command;
use vault_search::{PageDocument, TextField};

pub const PAGES: u64 = 2_900_000;
const TITLES: [&str; 16] = [
    "rust ownership and borrowing guide",
    "building accessible web applications",
    "understanding database query plans",
    "a practical guide to computer networks",
    "typescript patterns for maintainable software",
    "travel photography and light",
    "guide to chinese language learning",
    "designing resilient distributed systems",
    "home cooking seasonal vegetables",
    "linux performance tracing tutorial",
    "unicode text processing reference",
    "guide to browser security and privacy",
    "independent music and album reviews",
    "research papers on information retrieval",
    "hiking trails and local weather",
    "notes on history science and mathematics",
];

pub fn page(id: u64) -> PageDocument {
    let topic = (id.wrapping_mul(6364136223846793005).wrapping_add(1) >> 32) % 16;
    let mut title = TITLES[topic as usize].to_owned();
    // Controlled probes have independent exact match counts, including a ubiquitous
    // term. The remaining title/URL/enrichment fields give variable lengths and scores.
    title.push_str(" archive");
    if id % 317 == 0 {
        title.push_str(" webassembly");
    }
    if id == 1_200_001 {
        title.push_str(" raresentinel");
    }
    let domain = format!("site{}.example", id % 20000);
    let url = format!("https {domain} articles {} {id}", TITLES[topic as usize]);
    let mut p = PageDocument::new(id);
    p.text[TextField::Url as usize] = url.clone();
    p.text[TextField::Title as usize] = title.clone();
    p.text[TextField::NormalizedUrl as usize] = url.clone();
    p.text[TextField::NormalizedTitle as usize] = title.clone();
    if id % 5 == 0 {
        p.text[TextField::SearchTerms as usize] = "how to search browser history".into();
        p.text[TextField::NormalizedSearchTerms as usize] = "how to search browser history".into();
    }
    if id % 7 == 0 {
        p.text[TextField::Enrichment as usize]="reference documentation examples community tutorials implementation details and troubleshooting".into();
    }
    if id % 13 == 0 {
        p.text[TextField::Notes as usize] = "read again and compare with the project design".into();
    }
    if id % 11 == 0 {
        p.text[TextField::Tags as usize] = "reference reading".into();
    }
    if id % 19 == 0 {
        p.text[TextField::CjkGrams as usize] = "資料庫 搜尋指南".into();
    }
    p.compact_text = p.text.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    p.domain = domain;
    p.profile_ids = vec![1 + id % 3];
    if id % 10 == 0 {
        p.profile_ids.push(9);
    }
    p.newest_visit_ms = 1_700_000_000_000 + (id % 600_000) as i64 * 1000;
    p
}
pub fn disk_bytes(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            match entry.unwrap().metadata() {
                Ok(metadata) => metadata.len(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0, // Concurrent merge GC.
                Err(error) => panic!("cannot measure index disk usage: {error}"),
            }
        })
        .sum()
}
pub fn uptime() -> String {
    String::from_utf8(Command::new("uptime").output().unwrap().stdout).unwrap().trim().to_owned()
}
#[cfg(target_os = "macos")]
pub fn rss_mib() -> f64 {
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
pub fn rss_mib() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/statm").unwrap();
    let pages: f64 = stat.split_whitespace().nth(1).unwrap().parse().unwrap();
    // sysconf returns the page size and has no memory-safety preconditions.
    pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as f64 / 1048576.
}
pub fn peak_rss_mib() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // getrusage initializes the output on success and never retains its pointer.
    assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) }, 0);
    let bytes = unsafe { usage.assume_init() }.ru_maxrss as f64;
    if cfg!(target_os = "macos") { bytes / 1048576. } else { bytes / 1024. }
}
