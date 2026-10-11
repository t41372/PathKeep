//! Responsibilities: keep statements that open an ephemeral table out of the loops that run once
//! per row. Each run of one creates a temporary file (`temp_store=FILE`): about 150 µs on macOS,
//! and a write to disk (`ipc-performance.md` §14).
//! Not responsible for: speed, or the results of the reads it drives.
//! Failure modes: a statement run once per row (an import's visit or URL, an append's path, an
//! undo's URL) opens a temporary file each time: undoing an import of 1M URLs made a million.
//! Reads open a few per range (`generation IN (SELECT …)`, `IN (…)` lists), which is accepted.
//!
//! The fixture is large enough that a per-row statement runs hundreds of times in its step
//! (3,000 records, 400 URLs, about 165 path flows), while a read runs a fixed few times per range.

use vault_core::{
    AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, DayInsightsRequest,
    HistoryQuery, ScopedDateRangeRequest, TakeoutRequest, Vault,
};
use vault_store::Db;

const DAY_MS: i64 = 86_400_000;
const START_MS: i64 = 1_777_680_000_000;

fn export(folder: &std::path::Path, offset_days: i64, records: i64) {
    let rows = (0..records)
        .map(|n| {
            let page = n % 400;
            let url = if page % 40 == 3 {
                format!("https://www.google.com/search?q=topic+{page}")
            } else {
                format!("https://site-{}.example/page-{page}", page % 23)
            };
            let at = START_MS + offset_days * DAY_MS + n * 600_000;
            serde_json::json!({"url": url, "title": format!("Page {page}"), "time_usec": at * 1000})
        })
        .collect::<Vec<_>>();
    std::fs::create_dir_all(folder.join("Chrome")).unwrap();
    std::fs::write(
        folder.join("Chrome/BrowserHistory.json"),
        serde_json::json!({ "Browser History": rows }).to_string(),
    )
    .unwrap();
}

fn import(vault: &Vault, offset_days: i64, records: i64) -> i64 {
    let folder = tempfile::tempdir().unwrap();
    export(folder.path(), offset_days, records);
    let request =
        TakeoutRequest { source_path: folder.path().display().to_string(), dry_run: false };
    vault_core::import_takeout(vault, &request).unwrap().import_batch.unwrap().id
}

fn day(offset_days: i64) -> String {
    chrono::DateTime::from_timestamp_millis(START_MS + offset_days * DAY_MS)
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}

/// A statement that opens an ephemeral table more often than this in one step runs per row.
const PER_STEP: usize = 50;

/// Statements that opened an ephemeral table while `step` ran, by how often; those that ran
/// more than `PER_STEP` times are added to `per_row`.
fn report(vault: &Vault, per_row: &mut Vec<String>, step: &str) {
    for db in [Db::Archive, Db::Intelligence] {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for sql in vault.store().take_ephemeral_tables(db).unwrap() {
            *counts.entry(sql.split_whitespace().collect::<Vec<_>>().join(" ")).or_default() += 1;
        }
        let mut counts = counts.into_iter().collect::<Vec<_>>();
        counts.sort_by_key(|(_, runs)| std::cmp::Reverse(*runs));
        println!(
            "EPHEMERAL_TOTAL {step} {db:?} runs={} statements={}",
            counts.iter().map(|(_, runs)| runs).sum::<usize>(),
            counts.len()
        );
        for (sql, runs) in counts {
            println!("EPHEMERAL {step} {db:?} runs={runs} {}", &sql[..sql.len().min(220)]);
            if runs > PER_STEP {
                per_row.push(format!("{step} {db:?} runs={runs}: {sql}"));
            }
        }
    }
}

#[test]
fn no_statement_opens_a_temporary_file_per_row() {
    let vault = Vault::open_temp().unwrap().with_config(AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        insights_time_zone: Some("UTC".into()),
        ..AppConfig::default()
    });
    for db in [Db::Archive, Db::Intelligence] {
        vault.store().capture_query_plans(db, true).unwrap();
    }
    let mut per_row = Vec::new();
    let batch = import(&vault, 0, 3_000);
    report(&vault, &mut per_row, "import");
    vault_core::run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
    report(&vault, &mut per_row, "build");
    import(&vault, 25, 1_500);
    vault_core::run_core_intelligence(&vault, &CoreIntelligenceRebuildRequest::default()).unwrap();
    report(&vault, &mut per_row, "append");

    for (label, start, end) in [
        ("30-days", day(20), day(35)),
        ("year", day(-300), day(35)),
        ("all-time", "1900-01-01".into(), day(400)),
    ] {
        let scope = ScopedDateRangeRequest {
            date_range: DateRange { start: start.clone(), end: end.clone() },
            profile_id: None,
        };
        vault_core::intelligence::get_intelligence_primary_overview(&vault, &scope).unwrap();
        vault_core::intelligence::get_intelligence_secondary_overview(&vault, &scope).unwrap();
        report(&vault, &mut per_row, &format!("overviews-{label}"));
    }
    vault_core::get_day_insights(&vault, &DayInsightsRequest { date: day(10), profile_id: None })
        .unwrap();
    report(&vault, &mut per_row, "day");
    vault_core::list_history(&vault, HistoryQuery { limit: Some(50), ..HistoryQuery::default() })
        .unwrap();
    vault_core::list_history(
        &vault,
        HistoryQuery { q: Some("page".into()), limit: Some(50), ..HistoryQuery::default() },
    )
    .unwrap();
    report(&vault, &mut per_row, "history");
    vault_core::revert_import_batch(&vault, batch).unwrap();
    vault_core::restore_import_batch(&vault, batch).unwrap();
    report(&vault, &mut per_row, "undo-restore");
    assert!(
        per_row.is_empty(),
        "statements opening a temporary file per row:\n{}",
        per_row.join("\n")
    );
}
