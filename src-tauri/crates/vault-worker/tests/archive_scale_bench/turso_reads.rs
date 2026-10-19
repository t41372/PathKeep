//! Responsibilities: measure encrypted archive reads and full exports at the product scale.
//! Not responsible for: search, intelligence or extrapolating another machine's performance.
//! Failure modes: benchmarking a smaller/empty store, warm-only measurements, hidden export
//! truncation, unreported memory/load, excessive disk use or leftover benchmark data.
use std::{path::Path, time::Instant};
use vault_core::{AppConfig, ArchiveMode, ExportFormat, ExportRequest, HistoryQuery, Vault};
use vault_store::{Db, Priority, params};
const VISITS: i64 = 14_400_000;
const PAGES: i64 = 2_880_000;
const START: i64 = 1_600_000_000_000;
const STEP: i64 = 10_000;
const OPERATIONS: &[&str] = &[
    "timeline-first",
    "timeline-next",
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
/// The page visit `id` opens, as SQL over `id`. Visits are 10 s apart, 8,640 a day, and page `p`
/// belongs to site `p % 20000`. `PATHKEEP_BENCH_SHAPE` picks how a day's visits spread over sites:
///
/// - `cycle` (the default): visits walk the pages, and so the sites, in turn, so each of a day's
///   visits is to a different site: 8,640 site-day rows a day, the worst case for range reads.
/// - `habits`: closer to real browsing, where a site gets about 19 visits on a day it is visited
///   (the real data in `ipc-performance.md` §11). Half of a day's visits go to 50 sites visited
///   every day, the other half to 400 sites that change from day to day: about 450 site-day rows
///   a day. 4% of its visits are searches (`visit_page_and_opener`).
///
/// A data root keeps the shape it was seeded with; seed another shape into another root.
fn visit_page() -> String {
    match std::env::var("PATHKEEP_BENCH_SHAPE").as_deref() {
        Err(_) | Ok("cycle") => "(id-1)%2880000+1".to_owned(),
        Ok("habits") => {
            let slot = "(id%8640)";
            let site = format!(
                "(CASE WHEN {slot}%2=0 THEN ({slot}/2)%50 ELSE 50+((id/8640)*400+({slot}/2)%400)%19950 END)"
            );
            // One of the site's 143 pages, so its days have several pages each.
            format!("(CASE WHEN {site}=0 THEN 20000 ELSE {site} END)+20000*(({slot}/7)%143)")
        }
        Ok(other) => panic!("unknown PATHKEEP_BENCH_SHAPE {other}: cycle or habits"),
    }
}

/// In the `habits` shape every 25th visit is a search: 4%, the share on the real-data copy
/// (15,544 of 386,891 visits, `ipc-performance.md` §11). The `cycle` shape has none.
const SEARCH_EVERY: i64 = 25;
/// Searches made for the seeded visits and for the 100,000 an append may add after them.
const SEARCH_SLOTS: i64 = (VISITS + 100_000) / SEARCH_EVERY;

fn searches_seeded() -> bool {
    std::env::var("PATHKEEP_BENCH_SHAPE").as_deref() == Ok("habits")
}

/// The `url_id` and `from_visit` of visit `id`, as SQL over `id`, for `INSERT INTO visits`.
/// Every 144th visit is page 1, so one page has 100,000 visits. In the `habits` shape every
/// 25th other visit is the next search of `search_history::synthetic_searches` on Google
/// (`bench_search_slots` maps it to its page), and the two visits after it were each opened
/// from the one before, so a search's trail has pages. Other visits record no opener.
pub(super) fn visit_page_and_opener() -> String {
    if searches_seeded() {
        format!(
            "CASE WHEN id%144=0 THEN 1 WHEN id%{SEARCH_EVERY}=0 THEN (SELECT page_id FROM bench_search_slots WHERE slot=id/{SEARCH_EVERY}) ELSE {} END,CASE WHEN id%{SEARCH_EVERY} IN (1,2) THEN id-1 END",
            visit_page()
        )
    } else {
        format!("CASE WHEN id%144=0 THEN 1 ELSE {} END,NULL", visit_page())
    }
}

/// A Google search address for `query`, encoded as a browser sends it.
fn search_url(query: &str) -> String {
    let mut url = String::from("https://www.google.com/search?q=");
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                url.push(byte as char)
            }
            b' ' => url.push('+'),
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

/// Adds a page for every different search the visits make, after the archive pages, and the
/// slot table that gives each search visit its page. Returns how many pages it added.
fn seed_searches(vault: &Vault) -> i64 {
    let queries =
        vault_core::test_support::search_history::synthetic_searches(SEARCH_SLOTS as usize + 1);
    let mut pages = std::collections::HashMap::<&str, i64>::new();
    let mut slots = Vec::with_capacity(queries.len());
    for query in &queries {
        let next = PAGES + 1 + pages.len() as i64;
        slots.push(*pages.entry(query.as_str()).or_insert(next));
    }
    let mut distinct = pages.into_iter().collect::<Vec<_>>();
    distinct.sort_by_key(|(_, page)| *page);
    vault
        .store()
        .write(Db::Archive, Priority::Batch, |s| {
            s.execute(
                "CREATE TABLE bench_search_slots(slot INTEGER PRIMARY KEY, page_id INTEGER NOT NULL)",
                (),
            )?;
            Ok(())
        })
        .unwrap();
    for chunk in distinct.chunks(STEP as usize) {
        let rows = chunk.iter().map(|(query, page)| (query.to_string(), *page)).collect::<Vec<_>>();
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            for (query, page) in &rows {
                let url = search_url(query);
                let title = format!("{query} - Google Search");
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'google.com')",params![*page,url.as_str()])?;
                s.execute("INSERT INTO urls(id,page_id,url,title,visit_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(?1,?1,?2,?3,1,?4,'2020-09-13T12:26:40Z',?4,'2025-09-13T12:26:40Z',1,0,?1)",params![*page,url.as_str(),title.as_str(),START])?;
                s.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,updated_at) VALUES(?1,?2,?3,?4,?2,?3,?4,'','','')",params![*page,url.as_str(),title.as_str(),query.as_str()])?;
                s.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms) VALUES(?1,1,1,?2)",params![*page,START])?;
            }
            Ok(())
        }).unwrap();
    }
    for (first, chunk) in slots.chunks(STEP as usize).enumerate() {
        let first = (first * STEP as usize) as i64;
        let chunk = chunk.to_vec();
        vault
            .store()
            .write(Db::Archive, Priority::Batch, move |s| {
                for (offset, page) in chunk.iter().enumerate() {
                    s.execute(
                        "INSERT INTO bench_search_slots(slot,page_id) VALUES(?1,?2)",
                        params![first + offset as i64, *page],
                    )?;
                }
                Ok(())
            })
            .unwrap();
    }
    println!("BENCH seed_searches slots={} pages={}", slots.len(), distinct.len());
    distinct.len() as i64
}

/// The first visit seeded (`PATHKEEP_BENCH_FIRST_VISIT`, 1 by default). A later one keeps only
/// the last visits, at the same times: a range at the end of the data (the Insights bench's 30
/// days and year) reads the rows it would among 14.4M, and seeding and the full build take a
/// fraction of the time; all time covers less. For the Insights benchmark only: the archive
/// reads here count on all 14.4M.
pub(super) fn first_seeded_visit() -> i64 {
    std::env::var("PATHKEEP_BENCH_FIRST_VISIT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
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
    let search_pages = if !searches_seeded() {
        0
    } else if first_page <= PAGES {
        seed_searches(vault)
    } else {
        // A data root seeded before: its search pages are already there.
        vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT COUNT(*) FROM pages WHERE id>?1", [PAGES], |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap()
    };
    let first_visit = vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT COALESCE(MAX(id),0)+1 FROM visits", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap()
        .max(first_seeded_visit());
    let seeded = VISITS - first_seeded_visit() + 1;
    for first in (first_visit..=VISITS).step_by(STEP as usize) {
        let end = (first + STEP - 1).min(VISITS);
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute(&format!("WITH RECURSIVE ids(id) AS(VALUES(?1) UNION ALL SELECT id+1 FROM ids WHERE id<?2) INSERT INTO visits(id,url_id,from_visit,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT id,{},CAST(id AS TEXT),?3+id*10000,'2023-01-01T00:00:00Z',1,0 FROM ids",visit_page_and_opener()),params![first,end,START])?;
            Ok(())
        }).unwrap();
        if first % 1_000_000 == 1 {
            println!("BENCH seed_visits={end}");
            assert!(directory_bytes(&vault.paths().app_root) < 40_000_000_000);
        }
    }
    vault
        .store()
        .write(Db::Archive, Priority::Batch, move |s| {
            for (name, value) in
                [("visits", seeded), ("urls", PAGES + search_pages), ("sourceProfiles", 1)]
            {
                s.execute(
                    "UPDATE archive_totals SET value=?2 WHERE name=?1",
                    params![name, value],
                )?;
            }
            s.execute(
                "INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',?1) ON CONFLICT(name) DO UPDATE SET value=excluded.value",
                [seeded],
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
                    &vault.paths().app_root.join("export.jsonl"),
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
