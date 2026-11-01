//! Responsibilities: pin, through the real `process_profile_snapshot` pipeline on real Turso,
//! what a backup reads when the browser's file is no longer the one the last backup read, and
//! what it records when it cannot read a file to the end.
//! Not responsible for: the plan's checks on a bare table (`read_plan` tests).
//! Failure modes: a read of every row cut off part way is carried on in a file the browser has
//! made again since, from the old file's cursors: the pages under them are never read, so what
//! only the page row has (how often the address was typed, whether it is hidden) is missing while
//! the run says it succeeded; a checkpoint replay, whose rows an older copy of the file numbered,
//! moves the profile's cursor past rows the live file added since; a read that fails part way leaves its source batch saying it
//! covered nothing, with no reason, while the rows it saved point at it.
use super::*;

const T: i64 = 1_777_680_000_000; // 2026-05-01T00:00:00Z

fn page(id: i64, address: &str, typed: i64) -> ChromiumUrlRow {
    ChromiumUrlRow {
        id,
        url: address.to_string(),
        title: Some(address.to_string()),
        visit_count: 1,
        typed_count: typed,
        last_visit_unix_ms: T,
        hidden: false,
    }
}

fn typed_count(env: &ScenarioEnv, address: &str) -> i64 {
    env.open_archive()
        .query_row("SELECT typed_count FROM urls WHERE url=?1", [address.to_string()], |r| {
            Ok(r.get(0)?)
        })
        .expect("the page is archived")
}

#[test]
fn a_full_read_cut_off_in_a_file_made_again_since_starts_over() {
    let env = ScenarioEnv::new(None);
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    let before = ChromiumHistoryFixture::new()
        .add_url(page(1, "https://old.example/a", 0))
        .add_url(page(2, "https://old.example/b", 0))
        .add_url(page(3, "https://old.example/c", 0))
        .add_visit(visit_row(1, 1, T))
        .add_visit(visit_row(2, 2, T + 1_000))
        .add_visit(visit_row(3, 3, T + 2_000));
    seed_run(&env.open_archive(), 1);
    let cancel = vault_store::CancelToken::new();
    let stop = cancel.clone();
    // The first backup, a read of every row, stops as the process ending would, once its first
    // rows are saved.
    let cut_off = process_profile_snapshot(
        &env.vault,
        1,
        &snapshot_for_fixture(&before, profile()),
        false,
        true,
        &cancel,
        Some(Box::new(move |_| stop.cancel())),
    );
    assert!(cut_off.is_err(), "the backup stopped");
    // Then all history is cleared and Sync writes older visits back: other pages, under the
    // same low ids.
    let after = ChromiumHistoryFixture::new()
        .add_url(page(1, "https://new.example/x", 4))
        .add_url(page(2, "https://new.example/y", 6))
        .add_visit(visit_row(1, 1, T - 5_000))
        .add_visit(visit_row(2, 2, T - 4_000));
    run_one_ingest(&env, 2, &snapshot_for_fixture(&after, profile()), true);
    for (address, typed) in [("https://new.example/x", 4), ("https://new.example/y", 6)] {
        assert_eq!(typed_count(&env, address), typed, "{address}: its page row was read");
    }
}

#[test]
fn a_read_that_fails_part_way_says_in_its_source_batch_what_it_saved_and_why() {
    let env = ScenarioEnv::new(None);
    let fixture = ChromiumHistoryFixture::new()
        .add_url(page(1, "https://example.com/a", 0))
        .add_visit(visit_row(1, 1, T))
        .add_visit(visit_row(2, 1, T + 1_000));
    let snapshot =
        snapshot_for_fixture(&fixture, chromium_profile("chrome:Default", "Google Chrome"));
    // A visit time the parser cannot read: the pages are saved, the visits fail.
    rusqlite::Connection::open(&snapshot.history_path)
        .unwrap()
        .execute("UPDATE visits SET visit_time='not a time' WHERE id=2", [])
        .unwrap();
    let summary = run_one_ingest(&env, 1, &snapshot, true);
    assert!(
        matches!(summary.outcome, crate::models::ProfileOutcome::Failed { .. }),
        "{:?}",
        summary.outcome
    );
    let (coverage, notes) =
        crate::test_support::store::Probe::for_db(&env.vault, Db::SourceEvidence)
            .query_row("SELECT coverage_stats_json,notes_json FROM source_batches", (), |r| {
                Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?))
            })
            .unwrap();
    let coverage: serde_json::Value = serde_json::from_str(&coverage).unwrap();
    assert_ne!(coverage, serde_json::json!({}), "it says what it saved");
    assert!(notes.unwrap_or_default().contains("source-stream-failed"), "and why it ended");
}

#[test]
fn a_checkpoint_replay_leaves_the_profiles_cursor_where_the_live_file_left_it() {
    let env = ScenarioEnv::new(None);
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    let live = ChromiumHistoryFixture::new()
        .add_url(page(1, "https://example.com/a", 0))
        .add_visit(visit_row(1, 1, T))
        .add_visit(visit_row(2, 1, T + 1_000));
    run_one_ingest(&env, 1, &snapshot_for_fixture(&live, profile()), true);
    // A copy saved before the browser cleared its history: its rows run past the live cursor.
    let mut older = ChromiumHistoryFixture::new().add_url(page(1, "https://example.com/old", 0));
    for id in 1..=9 {
        older = older.add_visit(visit_row(id, 1, T - 100_000 + id));
    }
    run_one_ingest(&env, 2, &snapshot_for_fixture(&older, profile()), false);
    let cursor = env
        .vault
        .store()
        .read(Db::Archive, |s| super::super::parser::load_watermark(s, "chrome:Default"))
        .unwrap();
    assert_eq!(cursor.last_visit_id, 2, "the live file's cursor");
    assert_eq!(cursor.last_visit_url.as_deref(), Some("https://example.com/a"));
}
