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
            "full" => rebuild(&vault, true),
            "incremental" => {
                require_published_baseline(&vault).unwrap();
                rebuild(&vault, false);
            }
            operation => {
                require_published_baseline(&vault).unwrap();
                read(&vault, "cold", operation);
                read(&vault, "warm", operation);
            }
        }
        return;
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
    for operation in ["overview", "domain", "sessions"] {
        child(root.path(), operation);
    }
    let vault = super::turso_reads::open(root.path());
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        s.execute("WITH RECURSIVE ids(id) AS(VALUES(14400001) UNION ALL SELECT id+1 FROM ids WHERE id<14410000) INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT id,(id-1)%2880000+1,CAST(id AS TEXT),?1+id*10000,'2025-04-07T00:00:00Z',1,0 FROM ids",[START])?;
        s.execute("UPDATE archive_totals SET value=value+10000 WHERE name IN ('visits','sourceVisits:1')",())?;
        Ok(())
    }).unwrap();
    vault.close().unwrap();
    drop(vault);
    child(root.path(), "incremental");
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
fn rebuild(vault: &Vault, full: bool) {
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
    vault_core::run_core_intelligence_with_progress(
        vault,
        &CoreIntelligenceRebuildRequest {
            full_rebuild: full,
            limit: Some(2048),
            ..Default::default()
        },
        |p| {
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
        "INTELLIGENCE_BENCH {} elapsed_ms={} peak_rss_bytes={:?}",
        if full { "full" } else { "incremental-10000" },
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
