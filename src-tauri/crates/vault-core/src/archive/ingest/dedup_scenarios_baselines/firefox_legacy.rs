//! Responsibilities: pin how Firefox rows find version 1 visits after a file is renumbered,
//! through real SQLite sources, Turso archives and the public encrypted import API.
//! Not responsible for: Firefox Sync copies (`firefox_sync`) or choosing the read's rows.
//! Failure modes: a row takes a version 1 visit of another kind and is never saved; a visit gets
//! another kind's microseconds and the next full read adds a third visit; an import writes fewer
//! visits than its preview offers; an opener outside the read names a legacy visit of another
//! kind, even when its own kind is also in the archive.
use super::*;

const T: i64 = 1_777_680_000_000; // 2026-05-01T00:00:00Z
const ADDRESS: &str = "https://example.com/kinds";
const PROFILE: &str = "firefox:Default";

fn place(id: i64, address: &str) -> FirefoxPlaceRow {
    FirefoxPlaceRow {
        id,
        url: address.into(),
        title: Some(address.into()),
        visit_count: 1,
        hidden: false,
        last_visit_unix_ms: T,
    }
}

fn visit(id: i64, kind: i64) -> FirefoxVisitRow {
    FirefoxVisitRow {
        id,
        place_id: 1,
        visit_time_unix_ms: T,
        from_visit: None,
        visit_type: Some(kind),
    }
}

/// A file of (row id, kind, microseconds after `T`): Firefox keeps the microseconds that version
/// 1 did not. The fixture builder writes whole milliseconds, so set the native times afterward.
fn file(rows: &[(i64, i64, i64)]) -> ProfileSnapshot {
    let mut fixture = FirefoxPlacesFixture::new().add_place(place(1, ADDRESS));
    for &(id, kind, _) in rows {
        fixture = fixture.add_visit(visit(id, kind));
    }
    let snapshot = firefox_snapshot(&fixture, PROFILE);
    let db = rusqlite::Connection::open(&snapshot.history_path).unwrap();
    for &(id, _, micros) in rows {
        db.execute(
            "UPDATE moz_historyvisits SET visit_date=visit_date+?2 WHERE id=?1",
            rusqlite::params![id, micros],
        )
        .unwrap();
    }
    snapshot
}

/// Version 1 kept the kind, but neither the browser time nor a full-read mark.
fn version_one(env: &ScenarioEnv) {
    let archive = env.open_archive();
    archive.execute("UPDATE visits SET source_visit_time=NULL", ()).unwrap();
    archive.execute("UPDATE profile_watermarks SET last_full_scan_at=NULL", ()).unwrap();
}

fn times_and_kinds(env: &ScenarioEnv) -> Vec<(String, i64)> {
    env.open_archive()
        .query(
            "SELECT source_visit_time,transition_type FROM visits ORDER BY transition_type",
            (),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

#[test]
fn a_firefox_row_of_another_kind_does_not_take_a_version_one_visit() {
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &file(&[(7, 1, 0)]), true);
    version_one(&env);
    // The renumbered file holds a typed visit version 1 never saved, before the old link.
    let returned = file(&[(80, 2, 0), (81, 1, 0)]);
    let first = run_one_ingest(&env, 2, &returned, true);
    let saved = times_and_kinds(&env);
    let incremental = run_one_ingest(&env, 3, &returned, true);
    let full = run_one_ingest(&env, 4, &returned, false);
    let expected =
        vec![(format!("firefox-us:{}", T * 1_000), 1), (format!("firefox-us:{}", T * 1_000), 2)];
    assert!(matches!(first.outcome, crate::models::ProfileOutcome::Complete));
    assert_eq!(first.new_visits, 1, "the distinct kind is saved on the first read");
    assert_eq!(saved, expected);
    assert_eq!((incremental.new_visits, full.new_visits), (0, 0));
    assert_eq!(times_and_kinds(&env), expected);
}

#[test]
fn firefox_kinds_keep_their_own_microseconds_when_a_version_one_visit_is_matched() {
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &file(&[(7, 2, 500)]), true);
    version_one(&env);
    // Both times are inside the millisecond: neither is a Sync copy on its boundary.
    let returned = file(&[(80, 1, 123), (81, 2, 500)]);
    let first = run_one_ingest(&env, 2, &returned, true);
    let saved = times_and_kinds(&env);
    let full = run_one_ingest(&env, 3, &returned, false);
    let expected = vec![
        (format!("firefox-us:{}", T * 1_000 + 123), 1),
        (format!("firefox-us:{}", T * 1_000 + 500), 2),
    ];
    assert_eq!(first.new_visits, 1);
    assert_eq!(saved, expected, "each kind takes its own browser time");
    assert_eq!(full.new_visits, 0, "the next read of every row adds no third visit");
    assert_eq!(times_and_kinds(&env), expected);
}

/// The public Browser Direct entry points must offer and write the same visits in an encrypted
/// archive, including a kind the legacy archive has never seen.
fn browser_direct(
    env: &ScenarioEnv,
    snapshot: &ProfileSnapshot,
    dry_run: bool,
) -> crate::models::TakeoutInspection {
    let request = crate::models::BrowserHistoryImportRequest {
        source_path: snapshot.history_path.display().to_string(),
        dry_run,
        browser_family: Some("firefox".into()),
        profile_id: Some("browser-direct::firefox".into()),
        browser_name: Some("Firefox".into()),
        profile_name: Some("Imported".into()),
    };
    if dry_run {
        crate::inspect_browser_history(&env.vault, &request).unwrap()
    } else {
        crate::import_browser_history(&env.vault, &request).unwrap()
    }
}

#[test]
fn a_firefox_version_one_import_writes_the_distinct_kind_its_preview_offers() {
    let env = ScenarioEnv::new(Some([63; 32]));
    browser_direct(&env, &file(&[(7, 2, 0)]), false);
    version_one(&env);
    let returned = file(&[(80, 1, 0), (81, 2, 0)]);
    let preview = browser_direct(&env, &returned, true);
    let report = browser_direct(&env, &returned, false);
    assert_eq!(
        (preview.candidate_items, preview.duplicate_items, preview.same_visit_items),
        (2, 1, 0)
    );
    assert_eq!(
        (report.imported_items, report.duplicate_items, report.same_visit_items),
        (1, 1, 0),
        "the import writes the new visit its preview offered"
    );
    let expected =
        vec![(format!("firefox-us:{}", T * 1_000), 1), (format!("firefox-us:{}", T * 1_000), 2)];
    assert_eq!(times_and_kinds(&env), expected);
    let next = browser_direct(&env, &returned, false);
    assert_eq!((next.imported_items, next.duplicate_items), (0, 2));
    assert_eq!(times_and_kinds(&env), expected);
}

#[test]
fn a_firefox_opener_outside_the_read_can_only_name_a_version_one_visit_of_its_kind() {
    for has_link in [false, true] {
        let env = ScenarioEnv::new(None);
        let anchors = || {
            FirefoxPlacesFixture::new()
                .add_place(place(1, ADDRESS))
                .add_place(place(2, "https://example.com/first"))
                .add_place(place(3, "https://example.com/last"))
                .add_visit(FirefoxVisitRow {
                    place_id: 2,
                    visit_time_unix_ms: T + 1_000,
                    ..visit(1, 1)
                })
                .add_visit(FirefoxVisitRow {
                    place_id: 3,
                    visit_time_unix_ms: T + 2_000,
                    ..visit(100, 1)
                })
        };
        let mut legacy = anchors().add_visit(visit(7, 2));
        if has_link {
            legacy = legacy.add_visit(visit(8, 1));
        }
        run_one_ingest(&env, 1, &firefox_snapshot(&legacy, PROFILE), true);
        version_one(&env);
        // The old visits have expired before the first updated backup: it reads every row,
        // stamps the anchors and leaves the expired visits as version 1 saved them.
        run_one_ingest(&env, 2, &firefox_snapshot(&anchors(), PROFILE), true);
        let expected = env
            .open_archive()
            .query(
                "SELECT id FROM visits WHERE source_visit_time IS NULL AND transition_type=1",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )
            .unwrap()
            .into_iter()
            .next();
        // A rebuilt file happens to keep the first and last visit rows. Until the weekly full
        // read, the read plan cannot tell: the old link at row 50 is outside the recent window,
        // before the cursor. Only the new child is read; its opener must be found by its mark.
        let child_time = chrono::Utc::now().timestamp_millis() - 3_600_000;
        let rebuilt = anchors()
            .add_visit(visit(50, 1))
            .add_place(place(4, "https://example.com/child"))
            .add_visit(FirefoxVisitRow {
                place_id: 4,
                visit_time_unix_ms: child_time,
                from_visit: Some(50),
                ..visit(101, 1)
            });
        let summary = run_one_ingest(&env, 3, &firefox_snapshot(&rebuilt, PROFILE), true);
        assert_eq!(summary.new_visits, 1, "only the child was read");
        let parent = env
            .open_archive()
            .query_row("SELECT parent_visit_id FROM visits WHERE source_visit_id='101'", (), |r| {
                Ok(r.get::<Option<i64>>(0)?)
            })
            .unwrap();
        assert_eq!(parent, expected, "a link opener can name only the archived link");
        let legacy_kinds = env
            .open_archive()
            .query(
                "SELECT transition_type FROM visits WHERE source_visit_time IS NULL ORDER BY id",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )
            .unwrap();
        assert_eq!(legacy_kinds, if has_link { vec![2, 1] } else { vec![2] });
    }
}
