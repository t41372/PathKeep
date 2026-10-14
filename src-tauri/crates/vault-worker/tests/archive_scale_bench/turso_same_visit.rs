//! Responsibilities: measure what linking the same visit across sources costs a Takeout import
//! at the product scale (`docs/architecture/same-visit-across-sources.md`).
//! Not responsible for: the reads (`turso_reads`) or Insights (`turso_intelligence`).
//! Failure modes: measuring an import that looks nothing up (copies outside the browser's time
//! range), a warm cache passed off as cold, leftover benchmark data.
//!
//! The archive holds 14.4M visits of one browser (`turso_reads::seed`). Three Takeout imports of
//! `PATHKEEP_BENCH_SAME_VISITS` records (default 1M) go through the real import:
//! - records a year after the browser's last visit: they overlap no source and look nothing up;
//! - copies of the browser's latest visits: each looked up and written hidden;
//! - the same copies 5 s later: each looked up, nothing found within 1 s, written in full.
//!
//! The third minus the first is what looking up costs an import that overlaps the browser.
//! The second is cheaper than either: a hidden copy adds no totals or search documents.
//! `PATHKEEP_BENCH_SAME_VISIT_ONLY=no-overlap` runs the first alone, for timing the import's
//! writes (`WORK-IMPORT-BULK-WRITES`); `=again` then imports the same file a second time, what an
//! interrupted import run again pays for the records it already wrote (`WORK-TAKEOUT-RESUME`).

use super::turso_reads::{directory_bytes, open, seed};
use std::{path::Path, time::Instant};
use vault_core::{TakeoutRequest, Vault};
use vault_store::Db;

/// Only the import that looks nothing up: what the write path itself costs.
fn writes_only() -> bool {
    matches!(std::env::var("PATHKEEP_BENCH_SAME_VISIT_ONLY").as_deref(), Ok("no-overlap" | "again"))
}

/// The same import a second time, every record already there.
fn again() -> bool {
    std::env::var("PATHKEEP_BENCH_SAME_VISIT_ONLY").as_deref() == Ok("again")
}

fn records() -> i64 {
    std::env::var("PATHKEEP_BENCH_SAME_VISITS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1_000_000)
}

/// The browser's latest `count` visits (not an import's) as Takeout records, `shift_ms` later.
fn write_takeout(vault: &Vault, folder: &Path, count: i64, shift_ms: i64) {
    let visits = vault
        .store()
        .read(Db::Archive, move |s| {
            s.query(
                "SELECT u.url, v.visit_time_ms FROM visits v JOIN urls u ON u.id=v.url_id
                 WHERE v.source_profile_id=1 ORDER BY v.id DESC LIMIT ?1",
                [count],
                |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
            )
        })
        .unwrap();
    let records = visits
        .into_iter()
        .map(|(url, at)| {
            serde_json::json!({"url": url, "title": "Bench page", "time_usec": (at + shift_ms) * 1000})
        })
        .collect::<Vec<_>>();
    std::fs::create_dir_all(folder.join("Chrome")).unwrap();
    std::fs::write(
        folder.join("Chrome/BrowserHistory.json"),
        serde_json::to_vec(&serde_json::json!({ "Browser History": records })).unwrap(),
    )
    .unwrap();
}

fn import(vault: &Vault, folder: &Path, label: &str) {
    let started = Instant::now();
    let report = vault_core::import_takeout(
        vault,
        &TakeoutRequest { source_path: folder.display().to_string(), dry_run: false },
    )
    .unwrap();
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "BENCH same_visit {label}: {seconds:.1} s, {:.1} µs per record; imported {} same {} duplicate {}",
        seconds * 1e6 / records() as f64,
        report.imported_items,
        report.same_visit_items,
        report.duplicate_items
    );
}

#[test]
#[ignore = "encrypted 14.4M visits benchmark; imports three Takeout files and removes its data"]
fn takeout_import_links_the_same_visits_at_scale() {
    let root = tempfile::tempdir().unwrap();
    let vault = open(root.path());
    seed(&vault);
    let count = records();
    let apart = tempfile::tempdir().unwrap();
    // A year after the browser's last visit: no source has visits then.
    write_takeout(&vault, apart.path(), count, 366 * 86_400_000);
    import(&vault, apart.path(), "no overlap (no lookups)");
    if again() {
        import(&vault, apart.path(), "the same file again (every record already there)");
    }
    if writes_only() {
        finish(vault, root);
        return;
    }
    let copies = tempfile::tempdir().unwrap();
    write_takeout(&vault, copies.path(), count, 0);
    import(&vault, copies.path(), "copies of the browser's visits");
    // Inside the window `same_visit::candidates` checks, outside the one a copy is matched in.
    let later = tempfile::tempdir().unwrap();
    write_takeout(&vault, later.path(), count, 5_000);
    import(&vault, later.path(), "5 s after the browser's visits (looked up, not found)");
    finish(vault, root);
}

fn finish(vault: Vault, root: tempfile::TempDir) {
    vault.store().close().unwrap();
    drop(vault);
    println!("BENCH database_bytes={}", directory_bytes(root.path()));
    root.close().unwrap();
    println!("BENCH data_removed=true");
}
