//! Responsibilities: measure real encrypted conversion, subsequent index build and mode switch.
//! Not responsible for: fixture-generation performance or estimating target-machine results.
//! Failure modes: generated data saved under /state, setup RSS counted as conversion,
//! index work omitted, a smaller corpus silently reported as 14.4M.
use std::{path::Path, process::Command, time::Instant};
use vault_core::{AppConfig, ArchiveMode, Vault, project_paths_with_root};

#[path = "support/memory.rs"]
mod memory;

const PASSWORD: &str = "synthetic benchmark password";

#[test]
#[ignore = "14.4M encrypted SQLCipher conversion; run on Modal with --release --nocapture"]
fn encrypted_14_4m_conversion_and_mode_switch() {
    if let Ok(root) = std::env::var("PATHKEEP_CONVERSION_BENCH_ROOT") {
        measure(Path::new(&root));
        return;
    }
    let visits = std::env::var("PATHKEEP_CONVERSION_BENCH_VISITS")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(14_400_000i64);
    let pages = 2_900_000i64.min(visits);
    let root =
        tempfile::Builder::new().prefix("pathkeep-conversion-bench-").tempdir_in("/tmp").unwrap();
    generate(root.path(), visits, pages);
    for operation in ["convert", "mode"] {
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "encrypted_14_4m_conversion_and_mode_switch",
                "--ignored",
                "--nocapture",
            ])
            .env("PATHKEEP_CONVERSION_BENCH_ROOT", root.path())
            .env("PATHKEEP_CONVERSION_BENCH_OPERATION", operation)
            .status()
            .unwrap();
        assert!(status.success());
    }
    println!(
        "benchmark corpus: visits={visits} distinct_pages={pages}; generated under /tmp and removed on exit"
    );
}

fn measure(root: &Path) {
    let paths = project_paths_with_root(root);
    let config = vault_core::load_config(&paths).unwrap();
    let started = Instant::now();
    let memory = memory::PeakRss::start();
    if std::env::var("PATHKEEP_CONVERSION_BENCH_OPERATION").unwrap() == "convert" {
        let mut index_started = None;
        let mut copying_seconds = 0.0;
        let mut copying_rss = 0;
        let mut index_seconds = 0.0;
        let mut index_rss = 0;
        let mut last_progress = (String::new(), None, None, 0);
        let result = vault_convert_sqlcipher::convert(
            &paths,
            &config,
            Some(PASSWORD),
            Default::default(),
            |p| {
                let step =
                    (p.phase.clone(), p.database.clone(), p.table.clone(), p.processed / 1_000_000);
                if step != last_progress {
                    eprintln!(
                        "conversion phase={} database={:?} table={:?} processed={} total={}",
                        p.phase, p.database, p.table, p.processed, p.total
                    );
                    last_progress = step;
                }
                if p.phase == "indexing" && index_started.is_none() {
                    copying_seconds = started.elapsed().as_secs_f64();
                    copying_rss = memory.peak();
                    memory.reset();
                    index_started = Some(Instant::now());
                } else if p.phase == "publishing" {
                    index_seconds = index_started.unwrap().elapsed().as_secs_f64();
                    index_rss = memory.peak();
                }
            },
        )
        .unwrap();
        let total_seconds = started.elapsed().as_secs_f64();
        println!(
            "conversion_seconds={copying_seconds:.3} conversion_sampled_peak_rss_kib={copying_rss}"
        );
        println!(
            "conversion_total_seconds={total_seconds:.3} conversion_total_peak_rss_kib={} publication_seconds={:.3}",
            memory::kernel_peak(),
            total_seconds - copying_seconds - index_seconds
        );
        println!("index_seconds={index_seconds:.3} index_sampled_peak_rss_kib={index_rss}");
        result.vault.close().unwrap();
    } else {
        let (vault, _) = Vault::open_password(paths, config, PASSWORD, None).unwrap();
        let start = Instant::now();
        let result = vault_core::maintenance::switch_mode(
            &vault,
            Some(PASSWORD),
            ArchiveMode::Plaintext,
            None,
            Default::default(),
            |_| {},
        )
        .unwrap();
        println!(
            "mode_switch_seconds={:.3} mode_switch_peak_rss_kib={}",
            start.elapsed().as_secs_f64(),
            memory::kernel_peak()
        );
        result.vault.close().unwrap();
    }
}

fn generate(root: &Path, visits: i64, pages: i64) {
    let paths = project_paths_with_root(root);
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    vault_core::config::ensure_paths(&paths).unwrap();
    vault_core::save_config(&paths, &config).unwrap();
    vault_convert_sqlcipher::test_support::seed_reader_pages(
        &paths,
        &config,
        Some(PASSWORD),
        &[("https://example.com/page/1", "Topic page 1")],
    );
    let mut db =
        vault_convert_sqlcipher::test_support::open_archive(&paths, &config, Some(PASSWORD));
    for first in (2..=pages).step_by(10_000) {
        let tx = db.transaction().unwrap();
        {
            let mut insert=tx.prepare("INSERT INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(?1,?2,?3,5,0,?1,'then',?1,'then',1,1,?1)").unwrap();
            for id in first..=(first + 9999).min(pages) {
                insert
                    .execute(rusqlite::params![
                        id,
                        format!("https://site{}.example/page/{id}", id % 20_000),
                        format!("Topic {} archived page {id}", id % 1000)
                    ])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }
    for first in (2..=visits).step_by(10_000) {
        let tx = db.transaction().unwrap();
        {
            let mut insert=tx.prepare("INSERT INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id,source_visit_id) VALUES(?1,?2,1,?1,'then',1,?3)").unwrap();
            for id in first..=(first + 9999).min(visits) {
                insert
                    .execute(rusqlite::params![id, 1 + (id - 1) % pages, id.to_string()])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
}
