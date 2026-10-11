//! Responsibilities: measure one encrypted product-scale intelligence archive and remove it.
//! Not responsible for: provider calls, search measurements or CI latency thresholds.
//! Failure modes: warm caches masquerade as cold reads, rebuilds block interactive writes,
//! unreported load/RSS or batch progress, fixture duplication or retained data exceeding 40 GB.
use std::{path::Path, time::Instant};
use vault_core::{
    CoreIntelligenceRebuildRequest, DateRange, DomainDeepDiveRequest, PagedDateRangeRequest,
    ScopedDateRangeRequest, Vault,
};
use vault_store::{Db, Priority};
const MAX_BYTES: u64 = 40_000_000_000;
const WRITE_HEADROOM: u64 = 128 * 1024 * 1024;
const VISITS: i64 = 14_400_000;
const PAGES: i64 = 2_880_000;
const START: i64 = 1_600_000_000_000;

#[test]
#[ignore = "encrypted 14.4M intelligence benchmark, one archive below 40 GB, removed afterwards"]
fn encrypted_intelligence_at_scale() {
    super::COUNT_HEAP.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Ok(root) = std::env::var("PATHKEEP_INTELLIGENCE_BENCH_CHILD") {
        let vault = super::turso_reads::open(Path::new(&root));
        match std::env::var("PATHKEEP_INTELLIGENCE_BENCH_PHASE").unwrap().as_str() {
            "full" => rebuild(&vault, true, "full"),
            "incremental" => {
                require_published_baseline(&vault).unwrap();
                rebuild(&vault, false, "incremental-10000");
            }
            "categories" => {
                require_published_baseline(&vault).unwrap();
                rebuild(&vault, false, "categories");
            }
            operation if operation.starts_with("insights-") => {
                require_published_baseline(&vault).unwrap();
                insights(&vault, operation.trim_start_matches("insights-"));
            }
            "export-between-reads" => {
                require_published_baseline(&vault).unwrap();
                export_between_reads(&vault);
            }
            "site-days-sql" => {
                require_published_baseline(&vault).unwrap();
                site_days_sql(&vault);
            }
            operation => {
                require_published_baseline(&vault).unwrap();
                read(&vault, "cold", operation);
                read(&vault, "warm", operation);
            }
        }
        return;
    }
    // Every read the phases make, once on an empty archive, so a request the
    // command refuses fails now and not after seeding 14.4M visits.
    for range in ["30d", "1y", "all"] {
        insights(&Vault::open_temp_with_key(Some([7; 32])).unwrap(), range);
    }
    let root = BenchmarkRoot(
        std::env::var_os("PATHKEEP_INTELLIGENCE_BENCH_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| tempfile::tempdir().unwrap().keep()),
    );
    println!("INTELLIGENCE_BENCH root={}", root.path().display());
    let vault = super::turso_reads::open(root.path());
    super::turso_reads::seed(&vault);
    assert!(super::turso_reads::directory_bytes(root.path()) < MAX_BYTES);
    vault.close().unwrap();
    drop(vault);
    child(root.path(), "full");
    // `PATHKEEP_INTELLIGENCE_BENCH_OPERATIONS` (comma-separated) runs only those reads after the
    // build, and nothing after them.
    let only = std::env::var("PATHKEEP_INTELLIGENCE_BENCH_OPERATIONS").ok();
    let operations = only.as_deref().map_or_else(
        || {
            vec![
                "overview",
                "domain",
                "sessions",
                "insights-30d",
                "insights-1y",
                "insights-all",
                "export-between-reads",
            ]
        },
        |list| list.split(',').collect(),
    );
    for operation in operations {
        child(root.path(), operation);
    }
    if only.is_some() {
        drop(root);
        println!("INTELLIGENCE_BENCH data_removed=true");
        return;
    }
    let vault = super::turso_reads::open(root.path());
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        s.execute(&format!("WITH RECURSIVE ids(id) AS(VALUES(14400001) UNION ALL SELECT id+1 FROM ids WHERE id<14410000) INSERT INTO visits(id,url_id,from_visit,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT id,{},CAST(id AS TEXT),?1+id*10000,'2025-04-07T00:00:00Z',1,0 FROM ids",super::turso_reads::visit_page_and_opener()),[START])?;
        s.execute("UPDATE archive_totals SET value=value+10000 WHERE name IN ('visits','sourceVisits:1')",())?;
        Ok(())
    }).unwrap();
    vault.close().unwrap();
    drop(vault);
    child(root.path(), "incremental");
    // A site visited every day moves to another category: its visits' facts are rewritten and
    // every day's rollups made again, in place.
    let vault = super::turso_reads::open(root.path());
    vault.store().write(Db::Intelligence,Priority::Batch,|s| {
        s.execute("INSERT INTO site_dictionary_overrides(target_kind,target_value,domain_category,page_category,created_at,updated_at) VALUES('exact-domain','site1.test','docs','docs_page','','')",())?;
        Ok(())
    }).unwrap();
    vault.close().unwrap();
    drop(vault);
    child(root.path(), "categories");
    let bytes = super::turso_reads::directory_bytes(root.path());
    assert!(bytes < MAX_BYTES, "benchmark data exceeded 40 GB: {bytes}");
    println!("INTELLIGENCE_BENCH visits={VISITS} pages={PAGES} peak_final_data_bytes={bytes}");
    drop(root);
    println!("INTELLIGENCE_BENCH data_removed=true");
}
fn child(root: &Path, phase: &str) {
    let executable = std::env::current_exe().unwrap();
    let mut command = if phase == "full" {
        if let Some(directory) = std::env::var_os("PATHKEEP_INTELLIGENCE_PROFILE_DIR") {
            let mut command = std::process::Command::new("python3");
            command
                .arg(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../scripts/profile-native.py"),
                )
                .arg("--output")
                .arg(Path::new(&directory).join("full-rebuild.json"))
                .args(["--interval", "5", "--"])
                .arg(&executable);
            command
        } else {
            std::process::Command::new(&executable)
        }
    } else {
        std::process::Command::new(&executable)
    };
    let status = command
        .args([
            "--exact",
            "turso_intelligence::encrypted_intelligence_at_scale",
            "--ignored",
            "--nocapture",
        ])
        .env("PATHKEEP_INTELLIGENCE_BENCH_CHILD", root)
        .env("PATHKEEP_INTELLIGENCE_BENCH_PHASE", phase)
        .status()
        .unwrap();
    assert!(status.success(), "benchmark phase {phase} failed");
    assert!(super::turso_reads::directory_bytes(root) < MAX_BYTES);
}
fn rebuild(vault: &Vault, full: bool, label: &str) {
    load();
    let start = Instant::now();
    let (ready, wait) = std::sync::mpsc::channel();
    let owner = vault.clone();
    let waiter = std::thread::spawn(move || {
        wait.recv().unwrap();
        let began = Instant::now();
        vault_core::set_star(
            &owner,
            vault_core::SetStarRequest {
                entity_kind: vault_core::StarEntityKind::Url,
                entity_key: "https://site1.test/page/1".into(),
                source_profile: None,
            },
        )
        .unwrap();
        let delay = began.elapsed();
        println!(
            "INTELLIGENCE_BENCH interactive_write_wait_ms={:.3}",
            delay.as_secs_f64() * 1000.0
        );
        (delay, Instant::now())
    });
    let mut measured = false;
    let mut phase = String::new();
    let mut last_report = Instant::now();
    // The last event of the current phase, printed when the next phase starts: the work between
    // a phase's last event and the next phase's first is otherwise invisible.
    let mut last_event = (None, 0);
    vault_core::run_core_intelligence_with_progress(
        vault,
        &CoreIntelligenceRebuildRequest {
            full_rebuild: full,
            limit: Some(2048),
            ..Default::default()
        },
        |p| {
            if phase != p.phase && !phase.is_empty() {
                println!(
                    "INTELLIGENCE_BENCH phase_end={phase} processed={:?} elapsed_ms={}",
                    last_event.0, last_event.1
                );
            }
            if phase != p.phase || last_report.elapsed().as_secs() >= 60 {
                println!(
                    "INTELLIGENCE_BENCH phase={} processed={:?} total={:?} elapsed_ms={}",
                    p.phase,
                    p.processed_items,
                    p.total_items,
                    start.elapsed().as_millis()
                );
                phase = p.phase.clone();
                last_report = Instant::now();
            }
            last_event = (p.processed_items, start.elapsed().as_millis());
            if !measured
                && p.phase == "visit-derived-facts"
                && p.processed_items.is_some_and(|n| n >= 4096)
            {
                measured = true;
                ready.send(()).unwrap();
            }
            let bytes=super::turso_reads::directory_bytes(&vault.paths().app_root);
            if bytes>=MAX_BYTES-WRITE_HEADROOM {
                println!("INTELLIGENCE_BENCH aborted=data-budget phase={} processed={:?} data_bytes={bytes} elapsed_ms={} peak_rss_bytes={:?}",p.phase,p.processed_items,start.elapsed().as_millis(),super::turso_reads::peak_rss_bytes());
                anyhow::bail!("benchmark stopped before exceeding the 40 GB data budget");
            }
            Ok(())
        },
    )
    .unwrap();
    let completed = Instant::now();
    let (delay, written) = waiter.join().unwrap();
    assert!(written < completed, "interactive write must finish during the rebuild");
    println!("INTELLIGENCE_BENCH interactive_write_wait_ms={:.3}", delay.as_secs_f64() * 1000.0);
    println!(
        "INTELLIGENCE_BENCH {label} elapsed_ms={} peak_rss_bytes={:?}",
        start.elapsed().as_millis(),
        super::turso_reads::peak_rss_bytes()
    );
}
fn read(vault: &Vault, pass: &str, operation: &str) {
    load();
    let start = Instant::now();
    let range = DateRange { start: "2025-03-01".into(), end: "2025-04-07".into() };
    match operation {
        "overview" => {
            vault_core::intelligence::get_intelligence_primary_overview(
                vault,
                &ScopedDateRangeRequest { date_range: range, profile_id: None },
            )
            .unwrap();
        }
        "domain" => {
            vault_core::get_domain_deep_dive(
                vault,
                &DomainDeepDiveRequest {
                    pages_cursor: None,
                    searches_cursor: None,
                    page_size: None,
                    date_range: range,
                    profile_id: Some("chrome:Default".into()),
                    registrable_domain: "site1.test".into(),
                },
            )
            .unwrap();
        }
        "sessions" => {
            vault_core::get_sessions(
                vault,
                &PagedDateRangeRequest {
                    cursor: None,
                    date_range: range,
                    profile_id: Some("chrome:Default".into()),

                    page_size: 50,
                },
            )
            .unwrap();
        }
        _ => unreachable!(),
    }
    println!(
        "INTELLIGENCE_BENCH {pass} {operation} elapsed_ms={:.3} peak_rss_bytes={:?}",
        start.elapsed().as_secs_f64() * 1000.0,
        super::turso_reads::peak_rss_bytes()
    );
}
/// Every read Insights makes, or made in v0.3.0, for one range, twice each: the first call
/// in a new process, then again with its caches warm. "all" ends today, as the app's all-time
/// range does, so the primary and secondary overviews can come from the snapshot the full
/// rebuild warmed.
fn insights(vault: &Vault, range: &str) {
    load();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let (start, end) = match range {
        "30d" => ("2025-03-09", "2025-04-07".to_owned()),
        "1y" => ("2024-04-08", "2025-04-07".to_owned()),
        "all" => ("1900-01-01", today),
        _ => unreachable!(),
    };
    let request = serde_json::json!({
        "dateRange": { "start": start, "end": end },
        "profileId": null,
        "granularity": "day",
        "pageSize": 20,
        "stepCount": 3,
        "limit": 20,
        "sortBy": "visits",
    });
    macro_rules! time {
        ($name:literal, $call:path) => {
            for pass in ["first", "again"] {
                let request = serde_json::from_value(request.clone()).unwrap();
                let started = Instant::now();
                $call(vault, &request).unwrap();
                println!(
                    "INTELLIGENCE_BENCH insights range={range} {pass} {} elapsed_ms={:.1} peak_rss_bytes={:?}",
                    $name,
                    started.elapsed().as_secs_f64() * 1000.0,
                    super::turso_reads::peak_rss_bytes()
                );
            }
        };
    }
    time!("primary_overview", vault_core::intelligence::get_intelligence_primary_overview);
    time!("secondary_overview", vault_core::intelligence::get_intelligence_secondary_overview);
    // Where each overview's time goes, section by section.
    let scope: ScopedDateRangeRequest = serde_json::from_value(request.clone()).unwrap();
    let primary =
        vault_core::intelligence::get_intelligence_primary_overview(vault, &scope).unwrap();
    let secondary =
        vault_core::intelligence::get_intelligence_secondary_overview(vault, &scope).unwrap();
    for timing in primary.timings.iter().chain(&secondary.timings) {
        println!(
            "INTELLIGENCE_BENCH insights range={range} section {} elapsed_ms={}",
            timing.section_id, timing.duration_ms
        );
    }
    time!("digest", vault_core::get_digest_summary);
    time!("discovery_trend_day", vault_core::get_discovery_trend);
    time!("activity_mix_trend_day", vault_core::get_activity_mix_trend);
    time!("top_sites", vault_core::get_top_sites);
    time!("browsing_rhythm", vault_core::get_browsing_rhythm);
    time!("frequent_searches", vault_core::get_frequent_searches);
    time!("refind_pages", vault_core::get_refind_pages);
    time!("reopened_investigations", vault_core::get_reopened_investigations);
    time!("breadth_index", vault_core::get_breadth_index);
    time!("habit_patterns", vault_core::get_habit_patterns);
    time!("multi_browser_diff", vault_core::get_multi_browser_diff);
    time!("activity_mix", vault_core::get_activity_mix);
    time!("stable_sources", vault_core::get_stable_sources);
    time!("search_engine_ranking", vault_core::get_search_engine_ranking);
    time!("top_search_concepts", vault_core::get_top_search_concepts);
    time!("query_families", vault_core::get_query_families);
    time!("search_effectiveness", vault_core::get_search_effectiveness);
    time!("friction_signals", vault_core::get_friction_signals);
    time!("interrupted_habits", vault_core::get_interrupted_habits);
    time!("path_flows", vault_core::get_path_flows);
    time!("compare_sets", vault_core::get_compare_sets);
    time!("observed_interactions", vault_core::get_observed_interactions);
    time!("search_trails", vault_core::get_search_trails);
}

/// On a real archive (2026-10-08) the overviews stayed 3-4x slower after an export and the
/// app kept 3 GB resident. Reads, exports the whole app, reads again.
/// What grouping a range's site-day rows by site in SQL costs, against the shared read
/// (`intelligence::site_days`) the cards use: whether moving that grouping into SQL could bring a
/// year under the 30-day first batch. Both read the same rows; the SQL returns one row per site.
fn site_days_sql(vault: &Vault) {
    for (range, start) in [("30d", "2025-03-09"), ("1y", "2024-04-08")] {
        for pass in ["first", "again"] {
            let started = Instant::now();
            let sites = vault
                .store()
                .read(Db::Intelligence, move |s| {
                    s.query(
                        "SELECT r.registrable_domain, SUM(r.visit_count), SUM(r.search_count),
                                SUM(CASE WHEN r.visit_count>r.search_count THEN r.unique_urls ELSE 0 END),
                                COUNT(DISTINCT r.date_key), MIN(r.date_key), MAX(r.date_key),
                                MAX(r.last_visit_ms)
                         FROM domain_daily_rollups_rows r INDEXED BY idx_domain_daily_range
                         WHERE r.date_key>=?1 AND r.date_key<=?2
                           AND r.generation IN (SELECT daily FROM intelligence_generations)
                         GROUP BY r.registrable_domain",
                        vault_store::params![start, "2025-04-07"],
                        |r| Ok(r.get::<String>(0)?),
                    )
                })
                .unwrap()
                .len();
            println!(
                "INTELLIGENCE_BENCH site-days-sql range={range} {pass} sites={sites} elapsed_ms={:.1}",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        let request = serde_json::from_value(serde_json::json!({
            "dateRange": { "start": start, "end": "2025-04-07" },
            "profileId": null, "limit": 20, "sortBy": "visits",
        }))
        .unwrap();
        let started = Instant::now();
        vault_core::get_top_sites(vault, &request).unwrap();
        println!(
            "INTELLIGENCE_BENCH site-days-sql range={range} shared-read top_sites elapsed_ms={:.1}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
}

fn export_between_reads(vault: &Vault) {
    let request = ScopedDateRangeRequest {
        date_range: DateRange { start: "2025-03-09".into(), end: "2025-04-07".into() },
        profile_id: None,
    };
    let overviews = |label: &str| {
        for pass in ["first", "again"] {
            let started = Instant::now();
            vault_core::intelligence::get_intelligence_secondary_overview(vault, &request).unwrap();
            println!(
                "INTELLIGENCE_BENCH export-between-reads {label} {pass} secondary_overview elapsed_ms={:.1} rss_bytes={:?}",
                started.elapsed().as_secs_f64() * 1000.0,
                super::turso_reads::current_rss_bytes()
            );
        }
    };
    overviews("before");
    let target = vault.paths().app_root.parent().unwrap().join("bench-export.pathkeep");
    let started = Instant::now();
    vault_core::migration::native::export(
        vault,
        &target,
        "bench export",
        Default::default(),
        |_| {},
    )
    .unwrap();
    println!(
        "INTELLIGENCE_BENCH export-between-reads export elapsed_ms={} bytes={} rss_bytes={:?} peak_rss_bytes={:?}",
        started.elapsed().as_millis(),
        std::fs::metadata(&target).unwrap().len(),
        super::turso_reads::current_rss_bytes(),
        super::turso_reads::peak_rss_bytes()
    );
    std::fs::remove_file(&target).unwrap();
    overviews("after");
}
fn load() {
    let output = std::process::Command::new("uptime").output().unwrap();
    println!("INTELLIGENCE_BENCH {}", String::from_utf8_lossy(&output.stdout).trim());
}

struct BenchmarkRoot(std::path::PathBuf);
impl BenchmarkRoot {
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for BenchmarkRoot {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove benchmark data");
        println!("INTELLIGENCE_BENCH data_removed=true root={}", self.0.display());
    }
}

#[test]
fn skip_baseline_rebuild_requires_existing_intelligence_state() {
    for key in [None, Some([39; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let error = require_published_baseline(&vault).expect_err("missing native baseline");
        assert!(error.to_string().contains("existing Core Intelligence read model"));
    }
}

// A child phase can be rerun against a retained fixture; do not measure empty derived reads.
fn require_published_baseline(vault: &Vault) -> anyhow::Result<()> {
    let published = vault.store().read(Db::Intelligence, |s| {
        s.query_one("SELECT EXISTS(SELECT 1 FROM intelligence_generations WHERE facts>0 AND daily>0 AND navigation>0 AND aggregates>0)", (), |r| Ok(r.get::<i64>(0)? != 0))
    })?;
    anyhow::ensure!(published, "measurement requires an existing Core Intelligence read model");
    Ok(())
}
