//! Responsibilities: keep Firefox kinds distinct when a real backup resumes from a reused id.
//! Not responsible for: the plan's checks on bare tables (`read_plan` tests).
//! Failure modes: a deleted cursor row's id goes to a visit at the same address and microsecond
//! of another kind, older than the recent window; neither this backup nor the next reads it.
use super::*;

fn reused_cursor(before_kind: i64, after_kind: i64, anchor: bool) {
    let env = ScenarioEnv::new(Some([79; 32]));
    let at = chrono::Utc::now().timestamp_millis() - 3 * 86_400_000;
    let reused_id = if anchor { 7 } else { 1 };
    let mut fixture = FirefoxPlacesFixture::new()
        .add_place(FirefoxPlaceRow {
            id: 1,
            url: "https://example.com/reused".into(),
            title: Some("Reused".into()),
            visit_count: 1,
            hidden: false,
            last_visit_unix_ms: at,
        })
        .add_visit(FirefoxVisitRow {
            id: reused_id,
            place_id: 1,
            visit_time_unix_ms: at,
            from_visit: None,
            visit_type: Some(before_kind),
        });
    if anchor {
        fixture = fixture.add_visit(FirefoxVisitRow {
            id: 6,
            place_id: 1,
            visit_time_unix_ms: at - 10_000,
            from_visit: None,
            visit_type: Some(1),
        });
    }
    let source = firefox_snapshot(&fixture, "firefox:Default");
    assert_eq!(run_one_ingest(&env, 1, &source, true).new_visits, if anchor { 2 } else { 1 });
    // Firefox has no sequence: the next INSERT naturally gets the deleted newest row's id.
    let db = rusqlite::Connection::open(&source.history_path).unwrap();
    db.execute("DELETE FROM moz_historyvisits WHERE id=?1", [reused_id]).unwrap();
    db.execute(
        "INSERT INTO moz_historyvisits(place_id,visit_date,from_visit,visit_type) VALUES(1,?1,0,?2)",
        rusqlite::params![at * 1_000, after_kind],
    )
    .unwrap();
    assert_eq!(db.last_insert_rowid(), reused_id);
    drop(db);

    let saved = || {
        env.open_archive()
            .query(
                "SELECT source_visit_time,transition_type FROM visits ORDER BY source_visit_time,transition_type",
                (),
                |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
            )
            .unwrap()
    };
    let key = browser_history_parser::native_visits::firefox_key(at * 1_000);
    let mut expected = vec![(key.clone(), 1), (key, 2)];
    if anchor {
        expected
            .push((browser_history_parser::native_visits::firefox_key((at - 10_000) * 1_000), 1));
    }
    expected.sort();
    let next = run_one_ingest(&env, 2, &source, true);
    assert_eq!(next.new_visits, 1, "the next backup saves the other kind");
    assert!(matches!(next.outcome, crate::models::ProfileOutcome::Complete));
    assert_eq!(saved(), expected);
    let watermark = env
        .vault
        .store()
        .read(Db::Archive, |s| super::super::parser::load_watermark(s, "firefox:Default"))
        .unwrap();
    assert_eq!(watermark.last_visit_kind, Some(after_kind));
    assert_eq!(watermark.first_visit_kind, Some(if anchor { 1 } else { after_kind }));
    let first = browser_history_parser::native_visits::first_visit(
        &source.history_path,
        browser_history_parser::native_visits::VisitTable::Firefox,
    )
    .unwrap();
    assert!(
        matches!(
            super::super::read_plan::plan(
                &source.history_path,
                browser_history_parser::native_visits::VisitTable::Firefox,
                &watermark,
                first.as_ref(),
                chrono::Utc::now(),
            )
            .unwrap(),
            super::super::read_plan::ReadPlan::Recent { .. }
        ),
        "saved kinds allow the unchanged file to stay incremental"
    );
    assert_eq!(run_one_ingest(&env, 3, &source, true).new_visits, 0);
    assert_eq!(saved(), expected, "an unchanged backup keeps both exact events");
}

#[test]
fn a_firefox_link_cursor_reused_by_a_typed_visit_keeps_both() {
    reused_cursor(1, 2, false);
}

#[test]
fn a_firefox_typed_cursor_reused_by_a_link_keeps_both_with_an_unchanged_lowest_row() {
    reused_cursor(2, 1, true);
}
