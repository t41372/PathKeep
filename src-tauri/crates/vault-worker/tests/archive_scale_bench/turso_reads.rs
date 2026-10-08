//! Responsibilities: measure encrypted archive reads and full exports at the product scale.
//! Not responsible for: search, intelligence or extrapolating another machine's performance.
//! Failure modes: benchmarking a smaller/empty store, warm-only measurements, hidden export
//! truncation, unreported memory/load, excessive disk use or leftover benchmark data.
use std::{path::Path, time::Instant};
use vault_core::{
    AppConfig, ArchiveMode, BrowseDayInsightsRequest, ExportFormat, ExportRequest, HistoryQuery,
    Vault,
};
use vault_store::{Db, Priority, params};
const VISITS: i64 = 14_400_000;
const PAGES: i64 = 2_880_000;
const START: i64 = 1_600_000_000_000;
const STEP: i64 = 10_000;
const OPERATIONS: &[&str] = &[
    "timeline-first",
    "timeline-next",
    "day-insights",
    "url-detail-many-visits",
    "overview",
    "source-stats",
    "full-export",
];

/// Child phases open a new Store in a new process; the second pass reuses its caches.
#[test]
#[ignore = "encrypted 14.4M visits benchmark; emits measurements and removes its data"]
fn encrypted_archive_reads_at_scale() {
    if let Ok(root) = std::env::var("PATHKEEP_TURSO_READ_BENCH_CHILD") {
        let vault = open(Path::new(&root));
        let operation = std::env::var("PATHKEEP_TURSO_READ_BENCH_OPERATION")
            .expect("one operation per cold process");
        measure(&vault, "cold", &operation);
        measure(&vault, "warm", &operation);
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let vault = open(root.path());
    seed(&vault);
    vault.store().close().unwrap();
    drop(vault);
    let size = directory_bytes(root.path());
    assert!(size < 40_000_000_000, "benchmark exceeds 40 GB: {size}");
    println!("BENCH database_bytes={size} visits={VISITS} pages={PAGES}");
    for operation in OPERATIONS {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "turso_reads::encrypted_archive_reads_at_scale",
                "--ignored",
                "--nocapture",
            ])
            .env("PATHKEEP_TURSO_READ_BENCH_CHILD", root.path())
            .env("PATHKEEP_TURSO_READ_BENCH_OPERATION", operation)
            .status()
            .unwrap();
        assert!(status.success());
    }
    root.close().unwrap();
    println!("BENCH data_removed=true");
}
pub(super) fn open(root: &Path) -> Vault {
    Vault::open(
        vault_core::project_paths_with_root(root),
        AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: ArchiveMode::Encrypted,
            ..Default::default()
        },
        Some([83; 32]),
    )
    .unwrap()
}
pub(super) fn seed(vault: &Vault) {
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,profile_key) VALUES(1,'chrome','Default','/bench','','chrome:Default') ON CONFLICT(id) DO NOTHING",())?;
        Ok(())
    }).unwrap();
    let first_page = vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT COALESCE(MAX(id),0)+1 FROM pages", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    for first in (first_page..=PAGES).step_by(STEP as usize) {
        let end = (first + STEP - 1).min(PAGES);
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("WITH RECURSIVE ids(id) AS(VALUES(?1) UNION ALL SELECT id+1 FROM ids WHERE id<?2) INSERT INTO pages(id,url,canonical_url,registrable_domain) SELECT id,'https://site'||(id%20000)||'.test/page/'||id,'https://site'||(id%20000)||'.test/page/'||id,'site'||(id%20000)||'.test' FROM ids",params![first,end])?;
            s.execute("INSERT INTO urls(id,page_id,url,title,visit_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) SELECT id,id,url,'Archive page '||id,5,?3+id,'2020-09-13T12:26:40Z',?3+id,'2025-09-13T12:26:40Z',1,0,id FROM pages WHERE id>=?1 AND id<=?2",params![first,end,START])?;
            s.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,updated_at) SELECT id,url,'Archive page '||id,'',url,'Archive page '||id,'','','','' FROM pages WHERE id>=?1 AND id<=?2",params![first,end])?;
            s.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms) SELECT id,1,CASE WHEN id=1 THEN 100005 ELSE 5 END,?3+(id+11520000)*10000 FROM pages WHERE id>=?1 AND id<=?2 AND id%144!=0",params![first,end,START])?;
            Ok(())
        }).unwrap();
        if first % 1_000_000 == 1 {
            println!("BENCH seed_pages={end}");
            assert!(directory_bytes(&vault.paths().app_root) < 40_000_000_000);
        }
    }
    let first_visit = vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT COALESCE(MAX(id),0)+1 FROM visits", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    for first in (first_visit..=VISITS).step_by(STEP as usize) {
        let end = (first + STEP - 1).min(VISITS);
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("WITH RECURSIVE ids(id) AS(VALUES(?1) UNION ALL SELECT id+1 FROM ids WHERE id<?2) INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT id,CASE WHEN id%144=0 THEN 1 ELSE (id-1)%2880000+1 END,CAST(id AS TEXT),?3+id*10000,'2023-01-01T00:00:00Z',1,0 FROM ids",params![first,end,START])?;
            Ok(())
        }).unwrap();
        if first % 1_000_000 == 1 {
            println!("BENCH seed_visits={end}");
            assert!(directory_bytes(&vault.paths().app_root) < 40_000_000_000);
        }
    }
    vault
        .store()
        .write(Db::Archive, Priority::Batch, |s| {
            for (name, value) in [("visits", VISITS), ("urls", PAGES), ("sourceProfiles", 1)] {
                s.execute(
                    "UPDATE archive_totals SET value=?2 WHERE name=?1",
                    params![name, value],
                )?;
            }
            s.execute(
                "INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',?1) ON CONFLICT(name) DO UPDATE SET value=excluded.value",
                [VISITS],
            )?;
            Ok(())
        })
        .unwrap();
}
fn measure(vault: &Vault, pass: &str, operation: &str) {
    let load = std::process::Command::new("uptime").output().unwrap();
    println!("BENCH {pass} {operation} {}", String::from_utf8_lossy(&load.stdout).trim());
    match operation {
        "timeline-first" | "timeline-next" => {
            // The seeded timestamps increase with id, so this cursor needs no warming read.
            let cursor = (operation == "timeline-next")
                .then(|| format!("{}|{}", START + (VISITS - 149) * 10_000, VISITS - 149));
            let page = timed(pass, operation, || {
                vault_core::list_history(
                    vault,
                    HistoryQuery {
                        limit: Some(150),
                        cursor,
                        include_total: Some(false),
                        ..Default::default()
                    },
                )
                .unwrap()
            });
            assert_eq!(page.items.len(), 150);
            assert_eq!(
                page.items[0].id,
                if operation == "timeline-first" { VISITS } else { VISITS - 150 }
            );
        }
        "day-insights" => {
            let date =
                chrono::DateTime::from_timestamp_millis(START + VISITS * 10_000 - 86_400_000)
                    .unwrap()
                    .with_timezone(&chrono::Local)
                    .date_naive()
                    .to_string();
            let day = timed(pass, operation, || {
                vault_core::get_browse_day_insights(
                    vault,
                    &BrowseDayInsightsRequest { date, profile_id: None },
                )
                .unwrap()
            });
            assert!(day.total_pages > 0);
        }
        "url-detail-many-visits" => {
            let detail = timed(pass, operation, || {
                vault_core::get_url_detail(vault, "https://site1.test/page/1").unwrap()
            });
            assert_eq!(detail.total_visits, 100005);
        }
        "overview" => {
            let overview =
                timed(pass, operation, || vault_core::load_dashboard_snapshot(vault).unwrap());
            assert_eq!(overview.total_visits, VISITS as usize);
        }
        "source-stats" => {
            let sources = timed(pass, operation, || vault_core::load_source_stats(vault).unwrap());
            assert_eq!(sources.iter().map(|s| s.visit_count).sum::<i64>(), VISITS);
        }
        "full-export" => {
            let export = timed(pass, operation, || {
                vault_core::export_history(
                    vault,
                    ExportRequest {
                        export_id: None,
                        query: HistoryQuery::default(),
                        format: ExportFormat::Jsonl,
                    },
                )
                .unwrap()
            });
            assert_eq!(export.count, VISITS as usize);
            println!("BENCH {pass} export_process_peak_rss_bytes={:?}", peak_rss_bytes());
            println!(
                "BENCH {pass} export_bytes={}",
                std::fs::metadata(&export.path).unwrap().len()
            );
            assert!(directory_bytes(&vault.paths().app_root) < 40_000_000_000);
            std::fs::remove_file(export.path).unwrap();
        }
        _ => panic!("unknown benchmark operation: {operation}"),
    }
}
fn timed<T>(pass: &str, label: &str, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = work();
    println!("BENCH {pass} {label} elapsed_ms={:.3}", start.elapsed().as_secs_f64() * 1000.0);
    result
}
pub(super) fn directory_bytes(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            if m.is_dir() { directory_bytes(&e.path()) } else { m.len() }
        })
        .sum()
}

#[cfg(unix)]
pub(super) fn peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // The OS fills this POD structure when getrusage succeeds.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let peak = unsafe { usage.assume_init() }.ru_maxrss as u64;
    Some(if cfg!(target_os = "macos") { peak } else { peak * 1024 })
}
#[cfg(not(unix))]
pub(super) fn peak_rss_bytes() -> Option<u64> {
    None
}
/// Resident memory now, to see what a phase keeps after it ends (Linux only).
pub(super) fn current_rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}
