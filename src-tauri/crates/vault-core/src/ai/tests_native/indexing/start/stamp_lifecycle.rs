//! Exercises membership identity across native archive copy, replacement and removal.
//! The fixtures create real indexes; maintenance and move operations perform all file replacement.
use super::*;
use crate::ai::native_ledger::current_source_watermark;
use crate::{ArchiveMode, Vault, ai_queue, maintenance, migration::native};

fn back_up(vault: &Vault, id: i64) {
    crate::test_support::history::History::chrome(vault)
        .page(
            id,
            &format!("https://stamp.example/page/{id}"),
            Some("Stamp fixture"),
            1_775_000_000_000 + id * 1000,
        )
        .commit();
}

fn indexed(id: i64) -> Vault {
    let (_, config, db) = prepared_archive();
    let vault = db.vault.with_config(config.clone());
    crate::save_config(&crate::ControlGate::acquire(vault.paths()), &config).unwrap();
    back_up(&vault, id);
    run_backfill_chunk(
        &Runtime::new().unwrap(),
        &vault,
        &config,
        &embedding_provider(),
        &AiIndexRequest::default(),
        0,
    );
    assert_freshness(&vault, false);
    vault
}

fn stamp(vault: &Vault) -> i64 {
    let stamp = current_source_watermark(vault).unwrap();
    assert!(stamp < -2);
    stamp
}

fn pause(vault: &Vault) -> i64 {
    let request = AiIndexRequest { limit: Some(1), ..Default::default() };
    run_backfill_chunk(
        &Runtime::new().unwrap(),
        vault,
        vault.config(),
        &embedding_provider(),
        &request,
        0,
    );
    let job = crate::semantic_index::load_state(vault).unwrap().build.unwrap().job_id;
    ai_queue::pause_ai_job(vault, job).unwrap();
    ai_queue::mark_running_ai_job_cancelled(vault, job, None).unwrap();
    job
}

fn assert_freshness(vault: &Vault, stale: bool) {
    use crate::{AiIndexWarning, AiSemanticStaleness};
    let warning = crate::ai_index_status(vault).unwrap().warning_code;
    if stale {
        assert!(
            matches!(
                warning,
                Some(AiIndexWarning::IndexStale { reason: AiSemanticStaleness::Watermark })
            ),
            "{warning:?}"
        );
    } else {
        assert!(warning.is_none(), "{warning:?}");
    }
    assert_eq!(
        crate::ai::search_store::staleness(vault, "embed", "text-embedding-3-small").unwrap(),
        stale.then_some(AiSemanticStaleness::Watermark)
    );
}

fn assert_held_capture(vault: &Vault, job: i64, captured: i64) {
    assert_eq!(ai_queue::load_ai_job(vault, job).unwrap().state, "paused");
    let ai_queue::AiJobPayload::Index { cursor, .. } =
        ai_queue::load_ai_job_payload(vault, job).unwrap()
    else {
        panic!("expected an index cursor")
    };
    assert_eq!(cursor.scan.unwrap().source_watermark, captured);
    let identity = source_paths(vault, &embedding_provider()).vectors_dir.join("identity.json");
    let identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(identity).unwrap()).unwrap();
    assert_eq!(identity["source_watermark"].as_i64(), Some(captured));
}

#[test]
fn stamp_lifecycle_restore_with_the_same_key_forks_membership() {
    restored_fork(false);
}
#[test]
fn stamp_lifecycle_restore_converts_the_copy_to_live_encryption() {
    restored_fork(true);
}
fn restored_fork(change_mode: bool) {
    let original = indexed(1);
    let job = pause(&original);
    let captured = stamp(&original);
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("snapshot");
    maintenance::snapshot(&original, &copy).unwrap();
    let live = if change_mode {
        maintenance::switch_mode(
            &original,
            None,
            ArchiveMode::Encrypted,
            Some("new password"),
            Default::default(),
            |_| {},
        )
        .unwrap()
        .vault
    } else {
        original.clone()
    };
    back_up(&live, 2);
    let discarded = stamp(&live);
    assert_freshness(&live, true);
    let restored =
        maintenance::restore_snapshot(&live, &copy, None, Default::default()).unwrap().vault;
    assert_eq!(restored.config().archive_mode, live.config().archive_mode);
    assert_eq!(stamp(&restored), captured);
    assert_held_capture(&restored, job, captured);
    assert_freshness(&restored, false);
    back_up(&restored, 3);
    assert_freshness(&restored, true);
    assert_ne!(
        stamp(&restored),
        discarded,
        "one change on each side of a restore must not replay the discarded timeline"
    );
}

#[test]
fn stamp_lifecycle_mode_and_password_changes_preserve_holders() {
    let original = indexed(1);
    let job = pause(&original);
    let captured = stamp(&original);
    let encrypted = maintenance::switch_mode(
        &original,
        None,
        ArchiveMode::Encrypted,
        Some("first password"),
        Default::default(),
        |_| {},
    )
    .unwrap()
    .vault;
    assert_eq!(stamp(&encrypted), captured);
    assert_held_capture(&encrypted, job, captured);
    assert_freshness(&encrypted, false);
    maintenance::rewrap_password(&encrypted, "first password", "second password").unwrap();
    assert_eq!(stamp(&encrypted), captured);
    assert_freshness(&encrypted, false);
    back_up(&encrypted, 2);
    let changed = stamp(&encrypted);
    assert_ne!(changed, captured);
    assert_freshness(&encrypted, true);
    let plain = maintenance::switch_mode(
        &encrypted,
        Some("second password"),
        ArchiveMode::Plaintext,
        None,
        Default::default(),
        |_| {},
    )
    .unwrap()
    .vault;
    assert_eq!(stamp(&plain), changed);
    assert_held_capture(&plain, job, captured);
    assert_freshness(&plain, true);
    back_up(&plain, 3);
    assert_freshness(&plain, true);
}

fn assert_no_index(vault: &Vault) {
    let (ledgers, jobs) = vault
        .read(Db::Intelligence, |s| {
            Ok(s.query_one(
                "SELECT (SELECT COUNT(*) FROM ai_index_ledger),(SELECT COUNT(*) FROM ai_jobs)",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
            )?)
        })
        .unwrap();
    assert_eq!((ledgers, jobs), (0, 0));
    let state = crate::semantic_index::load_state(vault).unwrap();
    assert!(!state.chosen_complete && state.build.is_none());
    assert!(!crate::ai_index_status(vault).unwrap().ready);
    let old_source = vault
        .paths()
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name("embed", "text-embedding-3-small"));
    assert!(!old_source.exists(), "no previous lineage or generation in the live archive");
    assert_eq!(
        crate::ai::search_store::staleness(vault, "embed", "text-embedding-3-small").unwrap(),
        None
    );
}

#[test]
fn stamp_lifecycle_move_installs_membership_but_no_semantic_holders() {
    let source = indexed(1);
    let original = stamp(&source);
    let job = pause(&source);
    let output = tempfile::tempdir().unwrap();
    let bundle = output.path().join("move.pathkeep");
    native::export(&source, &bundle, "move password", Default::default(), |_| {}).unwrap();
    assert_held_capture(&source, job, original);
    assert_freshness(&source, false);
    back_up(&source, 2);
    let source_next = stamp(&source);
    assert_freshness(&source, true);
    let mut branches = vec![source_next];
    for populated in [false, true] {
        let destination = populated.then(|| indexed(21));
        if let Some(vault) = &destination {
            pause(vault);
        }
        let empty = tempfile::tempdir().unwrap();
        let paths = destination
            .as_ref()
            .map_or_else(|| crate::project_paths_with_root(empty.path()), |v| v.paths().clone());
        let options = crate::ApplyImportOptions {
            confirm_overwrite: populated,
            source_archive_key: Some("move password".into()),
        };
        let (report, installed) = native::import(
            &paths,
            destination.as_ref(),
            None,
            &bundle,
            &options,
            Default::default(),
        )
        .unwrap();
        assert_eq!(report.kept_previous_as_copy, populated);
        assert_eq!(stamp(&installed.vault), original);
        assert_no_index(&installed.vault);
        back_up(&installed.vault, 3);
        assert_no_index(&installed.vault);
        branches.push(stamp(&installed.vault));
    }
    let distinct: std::collections::HashSet<_> = branches.iter().collect();
    assert_eq!(
        distinct.len(),
        branches.len(),
        "exporter, empty import and overwrite must diverge: {branches:?}"
    );
}

#[test]
fn stamp_lifecycle_wipe_and_recreate_drops_holders_and_starts_a_new_history() {
    let original = indexed(1);
    pause(&original);
    let discarded = stamp(&original);
    let paths = original.paths().clone();
    let config = original.config().clone();
    crate::wipe_all_data(
        &paths,
        |_| Ok(Default::default()),
        || {
            original.close()?;
            Ok(())
        },
        |_| Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    crate::save_config(&crate::ControlGate::acquire(&paths), &config).unwrap();
    let new = Vault::open(paths, config, None).unwrap();
    assert_no_index(&new);
    assert_eq!(current_source_watermark(&new).unwrap(), -2);
    back_up(&new, 5);
    assert_ne!(
        stamp(&new),
        discarded,
        "recreating the same root must not repeat the old first stamp"
    );
}

#[test]
fn stamp_lifecycle_first_open_of_a_released_layout_keeps_legacy_coverage() {
    let (_root, paths) = crate::maintenance::creation::tests::released_archive();
    let config = base_config();
    crate::save_config(&crate::ControlGate::acquire(&paths), &config).unwrap();
    {
        let released = Vault::open_staged(paths.clone(), config.clone(), None).unwrap();
        back_up(&released, 1);
        // v0.5 had no token row. Publish in that representation before opening as v0.6.
        released
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("DELETE FROM archive_totals WHERE name='visitMembershipToken'", ())?;
                Ok(())
            })
            .unwrap();
        Runtime::new()
            .unwrap()
            .block_on(build_ai_index(&released, &embedding_provider(), &AiIndexRequest::default()))
            .unwrap();
        released.close().unwrap();
    }
    let promoted = Vault::open(paths, config, None).unwrap();
    assert_eq!(current_source_watermark(&promoted).unwrap(), (1_i64 << 32) ^ 1);
    assert_freshness(&promoted, false);
    back_up(&promoted, 2);
    assert!(stamp(&promoted) < -2);
    assert_freshness(&promoted, true);
}

#[test]
fn stamp_lifecycle_backup_import_undo_restore_and_replacement_change_membership() {
    let vault = indexed(1);
    let before = stamp(&vault);
    let takeout = tempfile::tempdir().unwrap();
    let chrome = takeout.path().join("Chrome");
    std::fs::create_dir(&chrome).unwrap();
    let url = "https://stamp.example/page/2";
    let time = 1_775_000_002_000_i64;
    let history = serde_json::json!({"Browser History":[{"url":url,"title":"Stamp fixture","time_usec":time*1000}]});
    std::fs::write(chrome.join("BrowserHistory.json"), serde_json::to_vec(&history).unwrap())
        .unwrap();
    let batch = crate::import_takeout(
        &vault,
        &crate::TakeoutRequest {
            source_path: takeout.path().display().to_string(),
            dry_run: false,
        },
    )
    .unwrap()
    .import_batch
    .unwrap()
    .id;
    let imported = stamp(&vault);
    assert_ne!(imported, before);
    assert_freshness(&vault, true);
    crate::revert_import_batch(&vault, batch).unwrap();
    let undone = stamp(&vault);
    assert_ne!(undone, imported);
    assert_freshness(&vault, true);
    crate::restore_import_batch(&vault, batch).unwrap();
    let restored = stamp(&vault);
    assert_ne!(restored, undone);
    assert_ne!(restored, imported);
    run_backfill_chunk(
        &Runtime::new().unwrap(),
        &vault,
        vault.config(),
        &embedding_provider(),
        &AiIndexRequest::default(),
        0,
    );
    assert_freshness(&vault, false);
    back_up(&vault, 2);
    let replaced = stamp(&vault);
    assert_ne!(replaced, restored);
    let visible = vault
        .read(Db::Archive, |s| {
            Ok(s.query_one("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |r| {
                Ok(r.get::<i64>(0)?)
            })?)
        })
        .unwrap();
    assert_eq!(visible, 2, "browser copy takes the imported copy's place");
    assert_freshness(&vault, true);
    back_up(&vault, 2);
    assert_eq!(stamp(&vault), replaced, "a duplicate backup changes no membership");
}
