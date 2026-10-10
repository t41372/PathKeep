//! Responsibilities: measure native archive reads, exports and Insights at product scale, and keep
//! export memory bounded as the archive grows.
//! Not responsible for: CI latency thresholds or extrapolating another machine's performance.
//! Failure modes: small fixtures hide full scans, exports truncate, caches obscure cold costs,
//! retained benchmark data grows without a bound, or concurrent tests contaminate allocator peaks.
//!
//! The ignored `turso_reads::encrypted_archive_reads_at_scale` test measures each native
//! read in its own new process, then repeats it warm, including a complete 14.4M-row
//! export and process peak RSS. Its fixture is encrypted and removed when the test ends.
//! `turso_intelligence::encrypted_intelligence_at_scale` does the same for an Insights rebuild.
//! Set CARGO_TARGET_DIR to the shared build directory before running cargo.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use vault_core::HistoryQuery;

/// Runs `call` `iterations` times and prints the median, the slowest, and the peak Rust heap
/// above the starting point (counted by [`CountingAllocator`]).
fn report_memory(label: &str, iterations: usize, mut call: impl FnMut()) -> usize {
    let mut samples = Vec::with_capacity(iterations);
    let mut peak_heap = 0usize;
    for _ in 0..iterations {
        let heap_start = HEAP_CURRENT.load(Ordering::Relaxed);
        HEAP_PEAK.store(heap_start, Ordering::Relaxed);
        let started = Instant::now();
        call();
        samples.push(started.elapsed());
        peak_heap = peak_heap.max(HEAP_PEAK.load(Ordering::Relaxed).saturating_sub(heap_start));
    }
    samples.sort();
    println!(
        "| {label:<55} | median {:>9.2} ms | max {:>9.2} ms | heap +{:>8.1} MiB |",
        samples[samples.len() / 2].as_secs_f64() * 1000.0,
        samples[samples.len() - 1].as_secs_f64() * 1000.0,
        peak_heap as f64 / (1024.0 * 1024.0),
    );
    peak_heap
}

// Large native measurements use process RSS and disable allocator accounting once per process.
static COUNT_HEAP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static HEAP_CURRENT: AtomicUsize = AtomicUsize::new(0);
static HEAP_PEAK: AtomicUsize = AtomicUsize::new(0);

/// The system allocator plus a live-bytes counter and a resettable peak, so a call's peak heap
/// can be read without the process-lifetime high-water mark that `ru_maxrss` gives.
struct CountingAllocator;

impl CountingAllocator {
    fn grew(size: usize) {
        let now = HEAP_CURRENT.fetch_add(size, Ordering::Relaxed) + size;
        HEAP_PEAK.fetch_max(now, Ordering::Relaxed);
    }
}

// SAFETY: every call is forwarded to `System`; the counters never affect the returned memory.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && COUNT_HEAP.load(Ordering::Relaxed) {
            Self::grew(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && COUNT_HEAP.load(Ordering::Relaxed) {
            Self::grew(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        if COUNT_HEAP.load(Ordering::Relaxed) {
            HEAP_CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
        }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let grown = unsafe { System.realloc(pointer, layout, new_size) };
        if !grown.is_null() && COUNT_HEAP.load(Ordering::Relaxed) {
            HEAP_CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
            Self::grew(new_size);
        }
        grown
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Failure mode (f): use the real export and allocator, comparing two archive sizes in an
/// otherwise idle integration-test process. This guards against collecting rows/strings again.
#[test]
fn export_heap_stays_bounded_as_archive_grows() {
    // The allocator is process-wide; another test opening a Vault must not count as export memory.
    if std::env::var_os("PATHKEEP_EXPORT_HEAP_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "export_heap_stays_bounded_as_archive_grows", "--nocapture"])
            .env("PATHKEEP_EXPORT_HEAP_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success(), "isolated export heap measurement failed");
        return;
    }
    let mut peaks = Vec::new();
    for visits in [10_000, 100_000] {
        let vault = native_export_fixture(visits);
        let heap = report_memory("bounded export heap", 1, || {
            let result = vault_core::export_history(
                &vault,
                vault_core::ExportRequest {
                    export_id: None,
                    query: HistoryQuery::default(),
                    format: vault_core::ExportFormat::Jsonl,
                },
            )
            .unwrap();
            assert_eq!(result.count as u64, visits);
            std::fs::remove_file(result.path).unwrap();
        });
        assert!(heap < 2 * 1024 * 1024, "export allocated {heap} bytes above baseline");
        peaks.push(heap);
    }
    assert!(peaks[1] <= peaks[0] + 256 * 1024, "export heap grew with archive size: {peaks:?}");
}

/// Independent canonical facts keep the allocator test on the same native read path
/// as desktop exports, with five visits per page at both corpus sizes.
fn native_export_fixture(visits: u64) -> vault_core::Vault {
    use vault_store::{Db, Priority, params};
    let vault = vault_core::Vault::open_temp().unwrap();
    vault.store().write(Db::Archive, Priority::Batch, move |s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,profile_key) VALUES(1,'chrome','Default','/fixture','','chrome:Default')", ())?;
        s.execute("WITH RECURSIVE ids(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM ids WHERE id<?1) INSERT INTO pages(id,url,canonical_url) SELECT id,'https://example.com/page/'||id,'https://example.com/page/'||id FROM ids", [visits as i64 / 5])?;
        s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) SELECT id,id,url,'Export page '||id,0,'',0,'',1,0 FROM pages", ())?;
        for first in (1..=visits as i64).step_by(10_000) {
            let last = (first + 9_999).min(visits as i64);
            s.execute("WITH RECURSIVE ids(id) AS (VALUES(?1) UNION ALL SELECT id+1 FROM ids WHERE id<?2) INSERT INTO visits(id,url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT id,(id-1)/5+1,1600000000000+id,'',1,0 FROM ids", params![first,last])?;
        }
        s.execute("UPDATE archive_totals SET value=CASE name WHEN 'sourceProfiles' THEN 1 WHEN 'urls' THEN ?1/5 WHEN 'visits' THEN ?1 ELSE value END", [visits as i64])?;
        s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',?1)", [visits as i64])?;
        Ok(())
    }).unwrap();
    vault
}

#[path = "archive_scale_bench/turso_reads.rs"]
mod turso_reads;

#[path = "archive_scale_bench/turso_intelligence.rs"]
mod turso_intelligence;

#[path = "archive_scale_bench/turso_same_visit.rs"]
mod turso_same_visit;
