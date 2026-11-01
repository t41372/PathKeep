//! Responsibilities: pin how a backup names openers and what a later read may change, through
//! the real `process_profile_snapshot` pipeline on real Turso.
//! Not responsible for: which rows a backup reads from a given watermark (`read_plan` tests).
//! Failure modes: a redirect's later hop, merged into its visit, becomes that visit's opener; an
//! opener later in the file than its child is never named; an older checkpoint replayed takes a
//! duration back; a version 1 visit's opener is lost once either visit gets its browser time;
//! the import preview counts one version 1 visit for two new ones, or a redirect into one as a
//! new visit, also in a profile the archive has never seen; a damaged time past year 9999 makes every later backup read every row; a backup
//! cut off after it saved a visit but before it named that visit's opener moves its mark past
//! the visit, so no later backup but the weekly full read reads it again; a visit's opener moves
//! to the one Chrome points it at after deleting the first, which the archive still has.
use super::*;
use crate::archive::ingest::identity::Previewed;

const T: i64 = 1_777_680_000_000; // 2026-05-01T00:00:00Z

fn url(id: i64, address: &str) -> ChromiumUrlRow {
    ChromiumUrlRow {
        id,
        url: address.to_string(),
        title: Some(address.to_string()),
        visit_count: 1,
        typed_count: 0,
        last_visit_unix_ms: T,
        hidden: false,
    }
}

fn opened_by(id: i64, url_id: i64, at: i64, opener: i64, duration_s: i64) -> ChromiumVisitRow {
    ChromiumVisitRow {
        from_visit: Some(opener),
        visit_duration_micros: Some(duration_s * 1_000_000),
        ..visit_row(id, url_id, at)
    }
}

fn parent_of(env: &ScenarioEnv, address: &str) -> (i64, Option<i64>) {
    env.open_archive()
        .query_row(
            "SELECT v.id,v.parent_visit_id FROM visits v JOIN urls u ON u.id=v.url_id WHERE u.url=?1",
            [address.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("one visit")
}

#[test]
fn a_merged_redirect_hop_is_not_its_own_opener_and_a_later_opener_is_named() {
    for key in [None, Some([85; 32])] {
        let env = ScenarioEnv::new(key);
        // Rows 1 and 2 are one visit (same address and time); row 2 names row 1, its own other
        // row. The real opener, row 3, comes after both in the file, as Sync writes it.
        let fixture = ChromiumHistoryFixture::new()
            .add_url(url(1, "https://example.com/child"))
            .add_url(url(2, "https://example.com/parent"))
            .add_visit(opened_by(1, 1, T, 3, 0))
            .add_visit(opened_by(2, 1, T, 1, 4))
            .add_visit(opened_by(3, 2, T - 1_000, 0, 1));
        let snapshot =
            snapshot_for_fixture(&fixture, chromium_profile("chrome:Default", "Google Chrome"));
        let summary = run_one_ingest(&env, 1, &snapshot, true);
        assert_eq!(summary.new_visits, 2, "two rows of one visit are one visit");
        let (child, opener) = parent_of(&env, "https://example.com/child");
        let (parent, _) = parent_of(&env, "https://example.com/parent");
        assert_ne!(opener, Some(child), "a visit is not its own opener");
        assert_eq!(opener, Some(parent));
    }
}

#[test]
fn a_replayed_older_checkpoint_does_not_take_a_duration_back() {
    let env = ScenarioEnv::new(None);
    let at = |duration_s| {
        ChromiumHistoryFixture::new()
            .add_url(url(1, "https://example.com/tab"))
            .add_visit(opened_by(1, 1, T, 0, duration_s))
    };
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    // The tab was still open when the checkpoint was taken (0 s); a later backup read 30 s.
    let older = snapshot_for_fixture(&at(0), profile());
    run_one_ingest(&env, 1, &snapshot_for_fixture(&at(30), profile()), true);
    run_one_ingest(&env, 2, &older, false);
    let duration: Option<i64> = env
        .open_archive()
        .query_row("SELECT visit_duration_ms FROM visits", (), |r| Ok(r.get(0)?))
        .unwrap();
    assert_eq!(duration, Some(30_000));
}

#[test]
fn a_version_one_opener_survives_either_visit_getting_its_browser_time() {
    let env = ScenarioEnv::new(None);
    // A version 1 archive: two visits, the child naming the parent by browser row id 7.
    let fixture = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/parent"))
        .add_url(url(2, "https://example.com/child"))
        .add_visit(opened_by(7, 1, T - 1_000, 0, 1))
        .add_visit(opened_by(8, 2, T, 7, 1));
    let snapshot =
        snapshot_for_fixture(&fixture, chromium_profile("chrome:Default", "Google Chrome"));
    run_one_ingest(&env, 1, &snapshot, true);
    let archive = env.open_archive();
    archive.execute("UPDATE visits SET source_visit_time=NULL, parent_visit_id=NULL", ()).unwrap();
    // The update's backfill runs before any backup stamps a visit again.
    crate::conversion::adopt_version_one_facts(&env.vault).unwrap();
    let (parent, _) = parent_of(&env, "https://example.com/parent");
    let (child, opener) = parent_of(&env, "https://example.com/child");
    assert_eq!(opener, Some(parent));
    // The child is read again and stamped; the parent has left the browser.
    archive
        .execute("UPDATE visits SET source_visit_time='chromium-us:1' WHERE id=?1", [child])
        .unwrap();
    let path = crate::intelligence::get_navigation_path(&env.vault, child).unwrap();
    assert_eq!(
        path.steps.iter().map(|step| step.visit_id).collect::<Vec<_>>(),
        vec![parent, child]
    );
}

#[test]
fn the_import_preview_counts_one_version_one_visit_once() {
    let env = ScenarioEnv::new(None);
    // Version 1 kept one of two visits less than a millisecond apart.
    let fixture = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/precise"))
        .add_visit(visit_row(1, 1, T));
    let snapshot =
        snapshot_for_fixture(&fixture, chromium_profile("chrome:Default", "Google Chrome"));
    run_one_ingest(&env, 1, &snapshot, true);
    env.open_archive().execute("UPDATE visits SET source_visit_time=NULL", ()).unwrap();
    let profile = env
        .open_archive()
        .query_row("SELECT id FROM source_profiles", (), |r| Ok(r.get::<i64>(0)?))
        .unwrap();
    let visit = |id: i64, micros: i64| browser_history_parser::ParsedVisit {
        source_visit_time: Some(format!("chromium-us:{micros}")),
        source_visit_id: id,
        source_url_id: 1,
        url: "https://example.com/precise".into(),
        title: None,
        visit_time_ms: T,
        visit_time_iso: String::new(),
        from_visit: None,
        transition: None,
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: None,
    };
    let flags = env
        .vault
        .store()
        .read(Db::Archive, move |s| {
            let mut preview = super::super::identity::Preview::default();
            let mut flags = Vec::new();
            for (id, micros) in [(1, 101), (2, 202)] {
                let row = visit(id, micros);
                let key = row.source_visit_time.clone().unwrap();
                flags.push(
                    preview.classify(s, Some(profile), "chromium", &row, &key)? != Previewed::New,
                );
            }
            Ok(flags)
        })
        .unwrap();
    assert_eq!(flags, vec![true, false], "the second visit is new: import writes it");
}

#[test]
fn the_import_preview_counts_a_redirect_into_a_version_one_visit_as_the_import_does() {
    let env = ScenarioEnv::new(None);
    let page = "https://example.com/redirected";
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    let once = ChromiumHistoryFixture::new().add_url(url(1, page)).add_visit(visit_row(1, 1, T));
    run_one_ingest(&env, 1, &snapshot_for_fixture(&once, profile()), true);
    let archive = env.open_archive();
    archive.execute("UPDATE visits SET source_visit_time=NULL", ()).unwrap();
    let source_profile =
        archive.query_row("SELECT id FROM source_profiles", (), |r| Ok(r.get::<i64>(0)?)).unwrap();
    // The browser now has that visit as two rows with one time, as Chrome writes a redirect.
    let hops = ChromiumHistoryFixture::new()
        .add_url(url(1, page))
        .add_visit(visit_row(1, 1, T))
        .add_visit(visit_row(2, 1, T));
    let snapshot = snapshot_for_fixture(&hops, profile());
    let rows = browser_history_parser::chromium::parse_history(
        &browser_history_parser::HistoryDatabaseSet {
            history_path: snapshot.history_path.clone(),
            favicons_path: None,
        },
        browser_history_parser::ChromiumReadCursor::default(),
    )
    .unwrap()
    .visits;
    let skipped = env
        .vault
        .store()
        .read(Db::Archive, move |s| {
            let mut preview = super::super::identity::Preview::default();
            rows.iter()
                .map(|row| {
                    let key = row.source_visit_time.clone().expect("a browser time");
                    preview
                        .classify(s, Some(source_profile), "chromium", row, &key)
                        .map(|previewed| previewed != Previewed::New)
                })
                .collect::<vault_store::Result<Vec<_>>>()
        })
        .unwrap();
    // Reading every row, as the first read after the update does, stamps the visit with the
    // first row's time and merges the second into it.
    let summary = run_one_ingest(&env, 2, &snapshot, false);
    assert_eq!(summary.new_visits, 0);
    let stamped = env
        .open_archive()
        .query_row("SELECT COUNT(*),COUNT(source_visit_time) FROM visits", (), |r| {
            Ok((r.get::<i64>(0)?, r.get::<i64>(1)?))
        })
        .unwrap();
    assert_eq!(stamped, (1, 1), "one visit, with the browser's time");
    assert_eq!(skipped, vec![true, true], "the preview skips what the read skips");
}

#[test]
fn a_damaged_time_past_year_9999_does_not_make_every_backup_read_every_row() {
    let env = ScenarioEnv::new(None);
    // Row 2's time is past what the parser keeps; it reads the row as year 9999.
    let damaged = browser_history_parser::types::MAX_VALID_UNIX_MS + 86_400_000;
    let fixture = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/a"))
        .add_visit(visit_row(1, 1, T))
        .add_visit(visit_row(2, 1, damaged))
        .add_visit(visit_row(3, 1, T + 1_000));
    let snapshot =
        snapshot_for_fixture(&fixture, chromium_profile("chrome:Default", "Google Chrome"));
    run_one_ingest(&env, 1, &snapshot, true);
    let watermark = env
        .vault
        .store()
        .read(Db::Archive, |s| super::super::parser::load_watermark(s, "chrome:Default"))
        .unwrap();
    assert_eq!(watermark.newest_visit_ms, T + 1_000, "the damaged time is not the newest visit");
    let table = browser_history_parser::native_visits::VisitTable::Chromium;
    let first =
        browser_history_parser::native_visits::first_visit(&snapshot.history_path, table).unwrap();
    let next = super::super::read_plan::plan(
        &snapshot.history_path,
        table,
        &watermark,
        first.as_ref(),
        chrono::Utc::now(),
    )
    .unwrap();
    assert!(matches!(next, super::super::read_plan::ReadPlan::Recent { .. }), "{next:?}");
}

#[test]
fn a_backup_cut_off_before_it_names_an_opener_leaves_the_next_backup_to_name_it() {
    let env = ScenarioEnv::new(None);
    let first = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/child"))
        .add_url(url(2, "https://example.com/parent"))
        .add_url(url(3, "https://example.com/earlier"))
        .add_visit(visit_row(1, 3, T));
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    run_one_ingest(&env, 1, &snapshot_for_fixture(&first, profile()), true);
    // Sync then writes two old visits after it, the child before its opener.
    let synced = first.add_visit(opened_by(2, 1, T + 2_000, 3, 0)).add_visit(opened_by(
        3,
        2,
        T + 1_000,
        0,
        0,
    ));
    let snapshot = snapshot_for_fixture(&synced, profile());
    seed_run(&env.open_archive(), 2);
    let cancel = vault_store::CancelToken::new();
    // Stops the backup as the process ending would: just after its rows are saved, before the
    // opener is named.
    let stop = cancel.clone();
    let cut_off = process_profile_snapshot(
        &env.vault,
        2,
        &snapshot,
        false,
        true,
        &cancel,
        Some(Box::new(move |_| stop.cancel())),
    );
    assert!(cut_off.is_err(), "the backup stopped");
    assert_eq!(parent_of(&env, "https://example.com/child").1, None, "saved, no opener yet");
    run_one_ingest(&env, 3, &snapshot, true);
    let (parent, _) = parent_of(&env, "https://example.com/parent");
    assert_eq!(parent_of(&env, "https://example.com/child").1, Some(parent));
}

#[test]
fn a_named_opener_stays_when_the_browser_points_the_visit_at_another() {
    let env = ScenarioEnv::new(None);
    // Within the last day, so the next backup reads these rows again.
    let at = chrono::Utc::now().timestamp_millis() - 3_600_000;
    let pages = || {
        ChromiumHistoryFixture::new()
            .add_url(url(1, "https://example.com/child"))
            .add_url(url(2, "https://example.com/opener"))
            .add_url(url(3, "https://example.com/start"))
            .add_visit(visit_row(1, 3, at))
    };
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    let opened = pages().add_visit(opened_by(2, 2, at + 1_000, 1, 0)).add_visit(opened_by(
        3,
        1,
        at + 2_000,
        2,
        0,
    ));
    run_one_ingest(&env, 1, &snapshot_for_fixture(&opened, profile()), true);
    let (opener, _) = parent_of(&env, "https://example.com/opener");
    assert_eq!(parent_of(&env, "https://example.com/child").1, Some(opener));
    // Deleting a visit, Chrome points its children at the visit that opened it.
    let deleted = pages().add_visit(opened_by(3, 1, at + 2_000, 1, 0));
    run_one_ingest(&env, 2, &snapshot_for_fixture(&deleted, profile()), true);
    assert_eq!(parent_of(&env, "https://example.com/child").1, Some(opener));
}

#[test]
fn two_version_one_visits_at_one_millisecond_keep_their_own_times_once_renumbered() {
    let env = ScenarioEnv::new(None);
    let profile = || chromium_profile("chrome:Default", "Google Chrome");
    let (link, typed) = (Some(0), Some(1));
    let visit = |id, transition| ChromiumVisitRow { transition, ..visit_row(id, 1, T) };
    // Two visits half a millisecond apart: a link and a typed one.
    let half_apart = |path: &std::path::Path, later: i64| {
        rusqlite::Connection::open(path)
            .unwrap()
            .execute("UPDATE visits SET visit_time=visit_time+500 WHERE id=?1", [later])
            .unwrap();
    };
    let before = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/a"))
        .add_visit(visit(1, link))
        .add_visit(visit(2, typed));
    let snapshot = snapshot_for_fixture(&before, profile());
    half_apart(&snapshot.history_path, 2);
    run_one_ingest(&env, 1, &snapshot, true);
    // As version 1 kept them: no browser times.
    env.open_archive().execute("UPDATE visits SET source_visit_time=NULL", ()).unwrap();
    // The browser numbers them again, the typed one first.
    let after = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/a"))
        .add_visit(visit(1, typed))
        .add_visit(visit(2, link));
    let snapshot = snapshot_for_fixture(&after, profile());
    half_apart(&snapshot.history_path, 1);
    run_one_ingest(&env, 2, &snapshot, true);
    let times = env
        .open_archive()
        .query(
            "SELECT transition_type,source_visit_time FROM visits ORDER BY transition_type",
            (),
            |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
        )
        .unwrap();
    let micros = |key: &str| key.trim_start_matches("chromium-us:").parse::<i64>().unwrap();
    assert_eq!(times.len(), 2, "still two visits: {times:?}");
    assert_eq!(
        micros(&times[1].1) - micros(&times[0].1),
        500,
        "the typed one is the later: {times:?}"
    );
}

/// A Chromium file as Browser Direct sees it: `request` previews or imports it into a profile the
/// archive has never seen.
fn browser_direct(
    env: &ScenarioEnv,
    history: &std::path::Path,
    dry_run: bool,
) -> crate::models::TakeoutInspection {
    let request = crate::models::BrowserHistoryImportRequest {
        source_path: history.display().to_string(),
        dry_run,
        browser_family: Some("chromium".into()),
        profile_id: Some("browser-direct::first".into()),
        browser_name: Some("Chrome".into()),
        profile_name: Some("Default".into()),
    };
    if dry_run {
        crate::inspect_browser_history(&env.vault, &request).unwrap()
    } else {
        crate::import_browser_history(&env.vault, &request).unwrap()
    }
}

#[test]
fn a_first_import_preview_counts_a_redirects_hops_as_one_visit() {
    // Chrome writes a redirect's hops with one time; the import keeps them as one visit even in
    // a profile the archive has never seen, and the preview says so before it.
    let file = ChromiumHistoryFixture::new()
        .add_url(url(1, "https://example.com/a"))
        .add_visit(visit_row(1, 1, T))
        .add_visit(ChromiumVisitRow { transition: Some(805306369), ..visit_row(2, 1, T) });
    let snapshot = snapshot_for_fixture(&file, chromium_profile("chrome:Unused", "Chrome"));
    let env = ScenarioEnv::new(None);
    let preview = browser_direct(&env, &snapshot.history_path, true);
    let report = browser_direct(&env, &snapshot.history_path, false);
    assert_eq!((report.imported_items, report.duplicate_items), (1, 1), "{report:?}");
    assert_eq!(
        (preview.candidate_items - preview.duplicate_items, preview.duplicate_items),
        (report.imported_items, report.duplicate_items),
        "the preview says what the import does"
    );
}
