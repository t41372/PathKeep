//! Responsibilities: pin, through the real `process_profile_snapshot` pipeline on real Turso,
//! which observation of a download the archive keeps when copies of the browser's file taken at
//! different times are read: a live backup, then a checkpoint saved before it.
//! Not responsible for: telling downloads apart (`writes::upsert_download`'s guid rule).
//! Failure modes: a replayed checkpoint, read after a backup saw the download finish, puts back
//! its older state (in progress, part of the bytes, the `.crdownload` path) and stamps it as
//! just recorded, and nothing reads it again once the browser forgets the download; an older
//! copy read first keeps a later backup from saving the finished state; a download only the
//! checkpoint has is not added.
use super::*;
use browser_history_fixtures::ChromiumDownloadRow;

const T: i64 = 1_777_680_000_000; // 2026-05-01T00:00:00Z

fn download(id: i64, guid: &str, received: i64, state: i64, path: &str) -> ChromiumDownloadRow {
    ChromiumDownloadRow {
        id,
        guid: Some(guid.to_string()),
        current_path: Some(path.to_string()),
        target_path: Some(format!("/Users/me/Downloads/{guid}.zip")),
        start_time_unix_ms: Some(T),
        received_bytes: Some(received),
        total_bytes: Some(100),
        state: Some(state),
        mime_type: Some("application/zip".to_string()),
        original_mime_type: None,
    }
}

fn copy_at(fixture: &ChromiumHistoryFixture, captured_at_ms: i64) -> ProfileSnapshot {
    let mut snapshot =
        snapshot_for_fixture(fixture, chromium_profile("chrome:Default", "Google Chrome"));
    snapshot.captured_at_ms = captured_at_ms;
    snapshot
}

fn stored(env: &ScenarioEnv, guid: &str) -> Option<(i64, i64, String)> {
    env.open_archive()
        .query_row(
            "SELECT received_bytes,state,current_path FROM downloads WHERE guid=?1",
            [guid.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()
}

const FINISHED: (i64, i64, &str) = (100, 1, "/Users/me/Downloads/g.zip");

fn finished(env: &ScenarioEnv) -> bool {
    stored(env, "g") == Some((FINISHED.0, FINISHED.1, FINISHED.2.to_string()))
}

fn in_progress() -> ChromiumHistoryFixture {
    ChromiumHistoryFixture::new()
        .add_download(download(1, "g", 10, 0, "/Users/me/Downloads/g.zip.crdownload"))
        .add_download(download(2, "h", 100, 1, "/Users/me/Downloads/h.zip"))
}

fn done() -> ChromiumHistoryFixture {
    ChromiumHistoryFixture::new().add_download(download(1, "g", 100, 1, FINISHED.2))
}

#[test]
fn a_replayed_checkpoint_keeps_the_newer_state_and_adds_what_only_it_has() {
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &copy_at(&done(), T + 60_000), true);
    assert!(finished(&env));
    // A checkpoint taken a minute before that backup, replayed after it.
    run_one_ingest(&env, 2, &copy_at(&in_progress(), T), false);
    assert!(finished(&env), "the finished download is not put back in progress");
    assert!(stored(&env, "h").is_some(), "the download only the checkpoint has is added");
}

#[test]
fn a_newer_copy_read_after_an_older_one_saves_its_state() {
    let env = ScenarioEnv::new(None);
    run_one_ingest(&env, 1, &copy_at(&in_progress(), T), false);
    assert_eq!(stored(&env, "g").map(|(received, state, _)| (received, state)), Some((10, 0)));
    run_one_ingest(&env, 2, &copy_at(&done(), T + 60_000), false);
    assert!(finished(&env));
}
