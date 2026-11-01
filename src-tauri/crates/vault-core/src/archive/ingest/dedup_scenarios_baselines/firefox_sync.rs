//! Responsibilities: pin how a Firefox visit Sync gives back is kept beside the one it probably
//! copies, through the real pipeline (`identity` rule 2).
//! Not responsible for: Chrome or Safari, whose times are not cut by Sync.
//! Failure modes: a device's own visit, given back by Sync after a reset with a new id and its
//! time cut to the millisecond, is counted a second time, or merged into the one it copies so
//! its own row is gone; another visit at that millisecond and address, of another kind, is taken
//! for the copy; the next backup of the same file adds the copy again; a visit the copy opened
//! points at the hidden copy, which no read shows; one hidden copy matched to two rows of one
//! read; the import preview and the import taking different visits for copies; an undone import
//! restored beside the copy that was shown in its place, both counted; two visits of different
//! kinds at one browser time (as Sync writes them, cut to the millisecond) taken for one, or the
//! visit one of them opened pointing at the other; a file of copies only previewed as nothing to
//! import, so its copies are never saved, or the import's report leaving them out.
use super::*;
use crate::archive::ingest::identity::Previewed;

const T: i64 = 1_777_680_000_000; // 2026-05-01T00:00:00Z
const ADDRESS: &str = "https://example.com/synced";

fn place(id: i64) -> FirefoxPlaceRow {
    FirefoxPlaceRow {
        id,
        url: ADDRESS.to_string(),
        title: Some("Synced".to_string()),
        visit_count: 1,
        hidden: false,
        last_visit_unix_ms: T,
    }
}

fn visit(id: i64, place_id: i64, visit_type: i64) -> FirefoxVisitRow {
    FirefoxVisitRow {
        id,
        place_id,
        visit_time_unix_ms: T,
        from_visit: None,
        visit_type: Some(visit_type),
    }
}

/// A Firefox file whose visit `id` is at `T` plus `micros` microseconds (the fixture writes whole
/// milliseconds; a live Firefox writes microseconds).
fn file(fixture: &FirefoxPlacesFixture, id: i64, micros: i64) -> ProfileSnapshot {
    let snapshot = firefox_snapshot(fixture, "firefox:Default");
    rusqlite::Connection::open(&snapshot.history_path)
        .unwrap()
        .execute(
            "UPDATE moz_historyvisits SET visit_date=visit_date+?2 WHERE id=?1",
            rusqlite::params![id, micros],
        )
        .unwrap();
    snapshot
}

/// The profile's visits: (browser time, the visit it is a hidden copy of, its opener), by id.
fn visits(env: &ScenarioEnv) -> Vec<(i64, String, Option<i64>, Option<i64>)> {
    env.open_archive()
        .query(
            "SELECT id,source_visit_time,same_as,parent_visit_id FROM visits ORDER BY id",
            (),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
}

fn shown(env: &ScenarioEnv) -> i64 {
    env.open_archive()
        .query_row("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |r| Ok(r.get(0)?))
        .unwrap()
}

/// Backs up a visit at `T` + 123 µs, then the file Sync rebuilt after a reset: the visit back as
/// row 20 at `T` exactly, of kind `copy_type`, and a visit it opened, twice.
fn after_a_sync_reset(copy_type: i64) -> ScenarioEnv {
    let env = ScenarioEnv::new(None);
    let own = FirefoxPlacesFixture::new().add_place(place(1)).add_visit(visit(10, 1, 1));
    run_one_ingest(&env, 1, &file(&own, 10, 123), true);
    let synced = FirefoxPlacesFixture::new()
        .add_place(place(4))
        .add_visit(visit(20, 4, copy_type))
        .add_visit(FirefoxVisitRow {
            from_visit: Some(20),
            visit_time_unix_ms: T + 5_000,
            ..visit(21, 4, 1)
        });
    let file = file(&synced, 20, 0);
    run_one_ingest(&env, 2, &file, true);
    run_one_ingest(&env, 3, &file, true);
    env
}

#[test]
fn a_visit_sync_gives_back_cut_to_the_millisecond_is_kept_and_counted_once() {
    let env = after_a_sync_reset(1);
    let all = visits(&env);
    assert_eq!(all.len(), 3, "the visit, its copy and the visit the copy opened: {all:?}");
    let (own, copy, opened) = (&all[0], &all[1], &all[2]);
    assert!(own.1.ends_with("123"), "the device's own time: {own:?}");
    assert_eq!(copy.2, Some(own.0), "the copy, with its own time, stands behind it: {copy:?}");
    assert_eq!(opened.3, Some(own.0), "what the copy opened points at the visit that counts");
    assert_eq!(shown(&env), 2, "counted once");
}

#[test]
fn a_visit_of_another_kind_at_that_millisecond_is_its_own_visit() {
    let env = after_a_sync_reset(2);
    assert!(visits(&env).iter().all(|visit| visit.2.is_none()));
    assert_eq!(shown(&env), 3);
}

/// The row `id` of a one-visit Firefox file at `T` plus `micros`, of kind 1.
fn one(id: i64, micros: i64) -> ProfileSnapshot {
    file(&FirefoxPlacesFixture::new().add_place(place(1)).add_visit(visit(id, 1, 1)), id, micros)
}

#[test]
fn one_hidden_copy_is_matched_to_one_row_of_a_read() {
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &one(10, 123), true);
    run_one_ingest(&env, 2, &one(20, 0), true);
    // Two more visits at that millisecond: one may be taken for a copy of the copy, not both.
    let two = FirefoxPlacesFixture::new()
        .add_place(place(1))
        .add_visit(visit(30, 1, 1))
        .add_visit(visit(31, 1, 1));
    let snapshot = firefox_snapshot(&two, "firefox:Default");
    rusqlite::Connection::open(&snapshot.history_path)
        .unwrap()
        .execute_batch(
            "UPDATE moz_historyvisits SET visit_date=visit_date+456 WHERE id=30;
             UPDATE moz_historyvisits SET visit_date=visit_date+789 WHERE id=31;",
        )
        .unwrap();
    let summary = run_one_ingest(&env, 3, &snapshot, true);
    assert_eq!(summary.new_visits, 1, "{summary:?}");
    assert_eq!(shown(&env), 2);
}

#[test]
fn the_import_preview_takes_the_same_rows_for_copies_as_the_import() {
    let env = ScenarioEnv::new(None);
    // A visit on the millisecond, and a copy of it with the device's precise time behind it.
    run_one_ingest(&env, 1, &one(10, 0), true);
    run_one_ingest(&env, 2, &one(20, 123), true);
    // A file with that copy's row, then another precise visit at the millisecond.
    let file = FirefoxPlacesFixture::new()
        .add_place(place(1))
        .add_visit(visit(40, 1, 1))
        .add_visit(visit(41, 1, 1));
    let snapshot = firefox_snapshot(&file, "firefox:Default");
    rusqlite::Connection::open(&snapshot.history_path)
        .unwrap()
        .execute_batch(
            "UPDATE moz_historyvisits SET visit_date=visit_date+123 WHERE id=40;
             UPDATE moz_historyvisits SET visit_date=visit_date+456 WHERE id=41;",
        )
        .unwrap();
    let profile = env
        .open_archive()
        .query_row("SELECT id FROM source_profiles", (), |r| Ok(r.get::<i64>(0)?))
        .unwrap();
    let row = |id: i64, micros: i64| browser_history_parser::ParsedVisit {
        source_visit_time: Some(format!("firefox-us:{}", T * 1_000 + micros)),
        source_visit_id: id,
        source_url_id: 1,
        url: ADDRESS.into(),
        title: None,
        visit_time_ms: T,
        visit_time_iso: String::new(),
        from_visit: None,
        transition: Some(1),
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: None,
    };
    let previewed_new = env
        .vault
        .store()
        .read(Db::Archive, move |s| {
            let mut preview = super::super::identity::Preview::default();
            let mut new = 0;
            for (id, micros) in [(40, 123), (41, 456)] {
                let visit = row(id, micros);
                let key = visit.source_visit_time.clone().unwrap();
                if preview.classify(s, Some(profile), "firefox", &visit, &key)? == Previewed::New {
                    new += 1;
                }
            }
            Ok(new)
        })
        .unwrap();
    let summary = run_one_ingest(&env, 3, &snapshot, false);
    assert_eq!(previewed_new, summary.new_visits, "{summary:?}");
}

/// Imports `snapshot` into the backup's own Firefox profile; returns its batch id.
fn import(env: &ScenarioEnv, snapshot: &ProfileSnapshot) -> i64 {
    import_into(env, snapshot, "firefox:Default")
}

/// Imports `snapshot` as a Firefox file into the profile `profile`; returns its batch id.
fn import_into(env: &ScenarioEnv, snapshot: &ProfileSnapshot, profile: &str) -> i64 {
    crate::import_browser_history(
        &env.vault,
        &crate::models::BrowserHistoryImportRequest {
            source_path: snapshot.history_path.display().to_string(),
            dry_run: false,
            browser_family: Some("firefox".into()),
            profile_id: Some(profile.into()),
            browser_name: Some("Firefox".into()),
            profile_name: Some("Default".into()),
        },
    )
    .unwrap();
    crate::load_import_batches(&env.vault, None, 16)
        .unwrap()
        .items
        .iter()
        .map(|b| b.id)
        .max()
        .unwrap()
}

/// What the archive counts: visible visits, and its total. Every import's visible count is
/// its share of them: none below zero, and together the visible imported visits.
fn counted(env: &ScenarioEnv) -> (i64, i64) {
    let (lowest, sum, imported): (i64, i64, i64) = env
        .open_archive()
        .query_row(
            "SELECT COALESCE(MIN(visible_items),0),
               COALESCE(SUM(CASE WHEN status NOT IN ('reverted','reverting') THEN visible_items END),0),
               (SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL AND import_batch_id IS NOT NULL)
             FROM import_batches",
            (),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!(lowest >= 0, "an import counted below zero");
    assert_eq!(sum, imported, "imports' visible counts");
    let total = env
        .open_archive()
        .query_row("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
            Ok(r.get::<i64>(0)?)
        })
        .unwrap();
    (shown(env), total)
}

#[test]
fn an_undone_import_restored_beside_its_sync_copy_counts_once() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    let copy = import(&env, &one(20, 0));
    assert_eq!(counted(&env), (1, 1), "the copy stands behind the visit");
    crate::revert_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (0, 0), "a Sync copy never stands in: it waits for its visit");
    crate::restore_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (1, 1), "restored, still one visit");
    // Both undone, the visit's own import last: restoring that one brings it back; restoring
    // the copy's then adds nothing.
    crate::revert_import_batch(&env.vault, copy).unwrap();
    crate::revert_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (0, 0));
    crate::restore_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (1, 1));
    crate::restore_import_batch(&env.vault, copy).unwrap();
    assert_eq!(counted(&env), (1, 1));
}

#[test]
fn a_sync_copys_import_restored_before_its_visits_waits_for_it() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    let copy = import(&env, &one(20, 0));
    crate::revert_import_batch(&env.vault, copy).unwrap();
    crate::revert_import_batch(&env.vault, own).unwrap();
    crate::restore_import_batch(&env.vault, copy).unwrap();
    assert_eq!(counted(&env), (0, 0), "the copy stays behind its undone visit");
    crate::restore_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (1, 1));
}

#[test]
fn two_visits_one_read_kept_apart_stay_apart_through_undo_and_restore() {
    let env = ScenarioEnv::new(None);
    // One file, one read: a visit on the millisecond and one 123 µs later. The read does not
    // take one for a copy of the other.
    let both = FirefoxPlacesFixture::new()
        .add_place(place(1))
        .add_visit(visit(10, 1, 1))
        .add_visit(visit(11, 1, 1));
    let snapshot = file(&both, 11, 123);
    let batch = import(&env, &snapshot);
    assert_eq!(counted(&env), (2, 2));
    crate::revert_import_batch(&env.vault, batch).unwrap();
    crate::restore_import_batch(&env.vault, batch).unwrap();
    assert_eq!(counted(&env), (2, 2));
}

#[test]
fn a_group_joined_through_a_copy_whose_import_was_undone_stays_one_visit() {
    let env = ScenarioEnv::new(None);
    let a = import(&env, &one(10, 123));
    let b = import(&env, &one(20, 0));
    let _c = import(&env, &one(30, 456));
    assert_eq!(counted(&env), (1, 1), "B copies A; C copies B");
    crate::revert_import_batch(&env.vault, b).unwrap();
    crate::revert_import_batch(&env.vault, a).unwrap();
    assert_eq!(counted(&env), (0, 0), "Sync copies wait for their visit");
    crate::restore_import_batch(&env.vault, a).unwrap();
    assert_eq!(counted(&env), (1, 1));
}

#[test]
fn a_copy_imported_while_its_visits_import_is_undone_counts() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    crate::revert_import_batch(&env.vault, own).unwrap();
    import(&env, &one(20, 0));
    assert_eq!(counted(&env), (1, 1), "not hidden behind a visit no read shows");
}

/// Imports one Takeout record of the visit at `T` (Chrome history JSON), the same visit as
/// another source's; returns its batch id.
fn takeout(env: &ScenarioEnv) -> i64 {
    takeout_at(env, T)
}

/// Imports one Takeout record at `at` (Unix ms) as its own export; returns its batch id.
fn takeout_at(env: &ScenarioEnv, at: i64) -> i64 {
    let root = tempfile::tempdir().unwrap();
    browser_history_fixtures::TakeoutBrowserHistoryFixture::new()
        .add_record(browser_history_fixtures::TakeoutBrowserRecord {
            url: ADDRESS.into(),
            title: Some("Synced".into()),
            visit_time_unix_ms: at,
            page_transition: Some("LINK".into()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        })
        .write(&root.path().join("Chrome/BrowserHistory.json"))
        .unwrap();
    crate::takeout::import_takeout(
        &env.vault,
        &crate::models::TakeoutRequest {
            source_path: root.path().display().to_string(),
            dry_run: false,
        },
    )
    .unwrap();
    crate::load_import_batches(&env.vault, None, 16)
        .unwrap()
        .items
        .iter()
        .map(|b| b.id)
        .max()
        .unwrap()
}

/// Which profile's visit counts for the visit at `T`.
fn counting_profile(env: &ScenarioEnv) -> String {
    env.open_archive()
        .query_row(
            "SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id
             WHERE v.reverted_at IS NULL",
            (),
            |r| Ok(r.get(0)?),
        )
        .unwrap()
}

#[test]
fn a_restored_browser_visit_stands_in_again_over_takeouts_copy() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    let copy = import(&env, &one(20, 0));
    takeout(&env);
    assert_eq!(counting_profile(&env), "firefox:Default");
    crate::revert_import_batch(&env.vault, own).unwrap();
    crate::revert_import_batch(&env.vault, copy).unwrap();
    assert_eq!(counting_profile(&env), "takeout::browser-history", "Takeout's copy counts now");
    crate::restore_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (1, 1));
    assert_eq!(counting_profile(&env), "firefox:Default", "the browser's record again");
}

#[test]
fn a_browser_visit_restored_after_takeouts_copy_stands_in_over_it() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    let copy = takeout(&env);
    crate::revert_import_batch(&env.vault, copy).unwrap();
    crate::revert_import_batch(&env.vault, own).unwrap();
    crate::restore_import_batch(&env.vault, copy).unwrap();
    crate::restore_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (1, 1));
    assert_eq!(counting_profile(&env), "firefox:Default");
}

#[test]
fn a_group_promoted_through_takeouts_copy_keeps_its_firefox_copy() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    let copy = import(&env, &one(20, 0));
    takeout(&env);
    crate::revert_import_batch(&env.vault, own).unwrap();
    crate::revert_import_batch(&env.vault, copy).unwrap();
    crate::restore_import_batch(&env.vault, own).unwrap();
    crate::restore_import_batch(&env.vault, copy).unwrap();
    assert_eq!(counted(&env), (1, 1));
    assert_eq!(counting_profile(&env), "firefox:Default");
}

/// A Firefox visit `ms` milliseconds after `T`, as its own import.
fn later(env: &ScenarioEnv, id: i64, ms: i64) -> i64 {
    import(env, &one(id, ms * 1_000))
}

#[test]
fn two_browser_visits_a_takeout_copy_lies_between_both_count_after_undo_and_restore() {
    // Takeout's copy at T is A's; C, half a second later, is another visit of the same browser.
    let env = ScenarioEnv::new(None);
    let a = import(&env, &one(10, 0));
    takeout(&env);
    let c = later(&env, 30, 500);
    assert_eq!(counted(&env), (2, 2));
    crate::revert_import_batch(&env.vault, a).unwrap();
    crate::revert_import_batch(&env.vault, c).unwrap();
    crate::restore_import_batch(&env.vault, c).unwrap();
    crate::restore_import_batch(&env.vault, a).unwrap();
    assert_eq!(counted(&env), (2, 2));
}

#[test]
fn a_browser_visit_regrouped_through_takeout_comes_back_on_its_own() {
    // A's Takeout copy takes A's place while A is undone, then a later visit C of the same
    // browser takes that copy's group: A, back, is not C's copy.
    let env = ScenarioEnv::new(None);
    let a = import(&env, &one(10, 0));
    let t = takeout(&env);
    let c = later(&env, 30, 500);
    crate::revert_import_batch(&env.vault, t).unwrap();
    crate::revert_import_batch(&env.vault, a).unwrap();
    crate::restore_import_batch(&env.vault, t).unwrap();
    crate::revert_import_batch(&env.vault, c).unwrap();
    crate::restore_import_batch(&env.vault, c).unwrap();
    crate::restore_import_batch(&env.vault, a).unwrap();
    assert_eq!(counted(&env), (2, 2));
}

#[test]
fn a_copy_of_a_copy_stays_with_its_visit_when_its_own_import_is_undone_and_restored() {
    let env = ScenarioEnv::new(None);
    import(&env, &one(10, 123));
    import(&env, &one(20, 0));
    let c = import(&env, &one(30, 456));
    crate::revert_import_batch(&env.vault, c).unwrap();
    crate::restore_import_batch(&env.vault, c).unwrap();
    assert_eq!(counted(&env), (1, 1));
}

#[test]
fn a_sync_copy_waits_for_its_visit_while_takeouts_copy_comes_and_goes() {
    let env = ScenarioEnv::new(None);
    let own = import(&env, &one(10, 123));
    import(&env, &one(20, 0));
    let t = takeout(&env);
    crate::revert_import_batch(&env.vault, own).unwrap();
    assert_eq!(counting_profile(&env), "takeout::browser-history");
    crate::revert_import_batch(&env.vault, t).unwrap();
    assert_eq!(counted(&env), (0, 0), "the Sync copy does not stand in");
    crate::restore_import_batch(&env.vault, own).unwrap();
    assert_eq!(counted(&env), (1, 1));
}

#[test]
fn two_browsers_visits_near_one_takeout_copy_both_count_after_undo_and_restore() {
    let env = ScenarioEnv::new(None);
    let a = import(&env, &one(10, 0));
    let b = takeout(&env);
    // Another browser profile, half a second later, at the same address.
    let other = one(30, 500_000);
    crate::import_browser_history(
        &env.vault,
        &crate::models::BrowserHistoryImportRequest {
            source_path: other.history_path.display().to_string(),
            dry_run: false,
            browser_family: Some("firefox".into()),
            profile_id: Some("firefox:Other".into()),
            browser_name: Some("Firefox".into()),
            profile_name: Some("Other".into()),
        },
    )
    .unwrap();
    assert_eq!(counted(&env), (2, 2));
    crate::revert_import_batch(&env.vault, b).unwrap();
    crate::revert_import_batch(&env.vault, a).unwrap();
    crate::restore_import_batch(&env.vault, b).unwrap();
    crate::restore_import_batch(&env.vault, a).unwrap();
    assert_eq!(counted(&env), (2, 2));
}

#[test]
fn two_takeout_copies_of_one_browser_visit_still_stand_in_for_each_other() {
    // As on main: the two Takeout records share a profile and a group, and are not Sync copies.
    let env = ScenarioEnv::new(None);
    let f = import(&env, &one(10, 0));
    let a = takeout_at(&env, T);
    takeout_at(&env, T + 500);
    crate::revert_import_batch(&env.vault, f).unwrap();
    crate::revert_import_batch(&env.vault, a).unwrap();
    assert_eq!(counted(&env), (1, 1), "the second record's import is still there");
    crate::restore_import_batch(&env.vault, f).unwrap();
    crate::restore_import_batch(&env.vault, a).unwrap();
    crate::revert_import_batch(&env.vault, a).unwrap();
    crate::revert_import_batch(&env.vault, f).unwrap();
    assert_eq!(counted(&env), (1, 1));
}

#[test]
fn copies_of_one_profile_that_no_read_took_for_sync_copies_still_stand_in() {
    // Two Firefox files imported into a Takeout profile key: ranked as Takeout, each linked to
    // the browser's visit as another source's copy, neither a Sync copy (they are 100 ms apart).
    let env = ScenarioEnv::new(None);
    let a = import_into(&env, &one(10, 0), "firefox:Main");
    let b = import_into(&env, &one(20, 100_000), "takeout::browser-history");
    import_into(&env, &one(30, 200_000), "takeout::browser-history");
    assert_eq!(counted(&env), (1, 1));
    crate::revert_import_batch(&env.vault, a).unwrap();
    crate::revert_import_batch(&env.vault, b).unwrap();
    assert_eq!(counted(&env), (1, 1), "the third import's copy stands in");
}

/// Previews `snapshot` as a Firefox file for `profile`, as the import card does.
fn preview(env: &ScenarioEnv, snapshot: &ProfileSnapshot, profile: &str) -> (usize, usize, usize) {
    let inspection = crate::inspect_browser_history(
        &env.vault,
        &crate::models::BrowserHistoryImportRequest {
            source_path: snapshot.history_path.display().to_string(),
            dry_run: true,
            browser_family: Some("firefox".into()),
            profile_id: Some(profile.into()),
            browser_name: Some("Firefox".into()),
            profile_name: Some("Default".into()),
        },
    )
    .unwrap();
    (inspection.candidate_items, inspection.duplicate_items, inspection.same_visit_items)
}

#[test]
fn two_kinds_of_visit_at_one_browser_time_are_two_visits() {
    // Sync cuts times to the millisecond, so a link and a typed visit of one page in that
    // millisecond come back at one time; Firefox keeps both, told apart by their kind.
    let file = FirefoxPlacesFixture::new()
        .add_place(place(1))
        .add_visit(visit(10, 1, 1))
        .add_visit(visit(11, 1, 2))
        .add_visit(FirefoxVisitRow {
            from_visit: Some(11),
            visit_time_unix_ms: T + 5_000,
            ..visit(12, 1, 1)
        });
    let snapshot = firefox_snapshot(&file, "firefox:Default");
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &snapshot, true);
    run_one_ingest(&env, 2, &snapshot, false);
    let kinds = env
        .open_archive()
        .query("SELECT id,transition_type,parent_visit_id FROM visits ORDER BY id", (), |r| {
            Ok((r.get::<i64>(0)?, r.get::<Option<i64>>(1)?, r.get::<Option<i64>>(2)?))
        })
        .unwrap();
    assert_eq!(kinds.len(), 3, "both kinds and the visit one opened, once each: {kinds:?}");
    assert_eq!(shown(&env), 3);
    let typed = kinds.iter().find(|(_, kind, _)| *kind == Some(2)).unwrap().0;
    assert_eq!(kinds[2].2, Some(typed), "the visit row 11 opened points at row 11's visit");
    // The preview counts what the import then writes, into a profile of its own.
    assert_eq!(preview(&env, &snapshot, "firefox:Other"), (3, 0, 0));
    import_into(&env, &snapshot, "firefox:Other");
    assert_eq!(preview(&env, &snapshot, "firefox:Other"), (3, 3, 0));
}

#[test]
fn a_file_of_sync_copies_only_is_previewed_as_copies_to_keep() {
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &one(10, 123), true);
    // The file Firefox rebuilt from Sync: only the visit back, on the millisecond.
    let copies = one(20, 0);
    assert_eq!(preview(&env, &copies, "firefox:Default"), (1, 0, 1), "not 'already there'");
    let report = crate::import_browser_history(
        &env.vault,
        &crate::models::BrowserHistoryImportRequest {
            source_path: copies.history_path.display().to_string(),
            dry_run: false,
            browser_family: Some("firefox".into()),
            profile_id: Some("firefox:Default".into()),
            browser_name: Some("Firefox".into()),
            profile_name: Some("Default".into()),
        },
    )
    .unwrap();
    assert_eq!(
        (report.imported_items, report.duplicate_items, report.same_visit_items),
        (0, 0, 1),
        "what the preview said"
    );
    assert_eq!(visits(&env).len(), 2, "the copy is saved");
    assert_eq!(shown(&env), 1, "and counted once");
    assert_eq!(preview(&env, &copies, "firefox:Default"), (1, 1, 0), "then it is there");
}
