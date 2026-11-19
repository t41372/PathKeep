//! Exercises repair races, offline publication and partial salvage with real archives.
//! Pair membership and layout admission have independent scenarios.
use super::*;
use vault_core::maintenance::{
    preview_conversation_repair as preview, repair_conversations as execute,
};
use vault_store::Db;

fn evidence(f: &Fixture) -> std::path::PathBuf {
    f.root.join(vault_layout::database_path(Db::AgentHistoryEvidence))
}

fn broken(f: &Fixture) -> Vault {
    f.session.close_vault().unwrap();
    fs::write(evidence(f), b"generation A").unwrap();
    f.reopen(false)
}

#[test]
fn generation_changed_at_close_survives() {
    for point in ["conversation-repair-closed", "native-install-prepared"] {
        let f = Fixture::new(false);
        let vault = broken(&f);
        let plan = preview(&vault, None, None).unwrap();
        let path = evidence(&f);
        vault_core::fault_inject::once_at(point, move || fs::write(path, b"generation B").unwrap());
        assert!(execute(&vault, None, None, &plan.token, Default::default()).is_err());
        assert_eq!(fs::read(evidence(&f)).unwrap(), b"generation B");
        assert!(!vault.paths().app_root.join(".pk-native-install.json").exists());
    }
}

#[test]
fn execute_consumes_the_prepared_preview() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    // A large deterministic opaque member measures the bytes actually retained,
    // independent of how much the test machine compresses native transcripts.
    let vault = broken(&f);
    let wal = std::path::PathBuf::from(format!("{}-wal", evidence(&f).display()));
    fs::write(&wal, vec![0x5a; 16 * 1024 * 1024]).unwrap();
    f.create_browser();
    let browser = vault.clone();
    vault_core::fault_inject::once_at("conversation-repair-preparing", move || {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tx.send(vault_core::run_backup(&browser, None).is_ok()).unwrap()
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(30))
                .expect("preparation held off browsing backup")
        );
    });
    let plan = preview(&vault, None, None).unwrap();
    assert!(fs::read_dir(&vault.paths().staging_dir).unwrap().count() > 0);
    let start = std::rc::Rc::new(std::cell::Cell::new(None));
    let last = start.clone();
    vault_core::fault_inject::once_at("conversation-repair-preparing", || {
        panic!("execute prepared a second time")
    });
    vault_core::fault_inject::once_at("conversation-repair-lease-start", move || {
        last.set(Some(std::time::Instant::now()))
    });
    let duration = std::rc::Rc::new(std::cell::Cell::new(None));
    let measured = duration.clone();
    vault_core::fault_inject::once_at("conversation-repair-lease-end", move || {
        measured.set(Some(start.get().unwrap().elapsed()))
    });
    let request = serde_json::from_value(serde_json::json!({"previewToken":plan.token})).unwrap();
    vault_worker::data_kinds::repair::execute(&f.session, request, |_| {}).unwrap();
    let bytes = vault_core::maintenance::list_snapshots(vault.paths()).unwrap()[0].size_bytes;
    eprintln!("exclusive session lease {:?}; displaced {} bytes", duration.get().unwrap(), bytes);
}

fn bad_digest(f: &Fixture) -> (Vault, i64) {
    let _corpus = Corpus::seed(f);
    let vault = f.vault();
    let id = vault.agent_history().unwrap().list_sessions(&Default::default()).unwrap().items[0]
        .revision_id;
    let copy = vault.paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots).join("good");
    vault_core::maintenance::snapshot(&vault, &copy).unwrap();
    f.session.close_vault().unwrap();
    rusqlite::Connection::open(evidence(f))
        .unwrap()
        .execute("UPDATE ah_revisions SET digest=lower(hex(zeroblob(32))) WHERE id=?1", [id])
        .unwrap();
    (f.reopen(false), id)
}

#[test]
fn retry_rechecks_failed_revision_and_good_copy_is_usable() {
    let f = Fixture::new(false);
    let (vault, id) = bad_digest(&f);
    assert!(vault.agent_history().unwrap().write_raw(id, &mut std::io::sink()).is_err());
    assert!(vault.retry_conversations().is_err(), "retry accepted the failed checksum");
    let copy = vault.paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots).join("good");
    assert_eq!(preview(&vault, Some(&copy), None).unwrap().conversations, "4");

    // Correct just the reported revision. An unrelated, structurally sound bad
    // digest must first surface when its own bytes are read.
    let replacement = f.sandbox.path().join("evidence-with-other-damage.db");
    fs::copy(copy.join(vault_layout::database_path(Db::AgentHistoryEvidence)), &replacement)
        .unwrap();
    let db = rusqlite::Connection::open(&replacement).unwrap();
    let other:i64=db.query_row("SELECT id FROM ah_revisions WHERE state='complete' AND bytes_retained=1 AND id<>?1 LIMIT 1",[id],|r|r.get(0)).unwrap();
    db.execute("UPDATE ah_revisions SET digest=lower(hex(zeroblob(32))) WHERE id=?1", [other])
        .unwrap();
    drop(db);
    fs::rename(replacement, evidence(&f)).unwrap();
    vault.retry_conversations().unwrap();
    let archive = vault.agent_history().unwrap();
    archive.write_raw(id, &mut std::io::sink()).unwrap();
    assert!(archive.write_raw(other, &mut std::io::sink()).is_err());
    let failed = vault.conversation_availability().unwrap_err();
    assert!(
        matches!(failed.cause,vault_store::Error::RetainedBytes{revision,..} if revision==other)
    );
}

#[test]
fn conversion_discovers_bad_bytes_before_publishing() {
    for switch in [false, true] {
        let f = Fixture::new(false);
        let (vault, id) = bad_digest(&f);
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = events.clone();
        vault_worker::data_kinds::backups::register_recovery_events(
            &f.session,
            Arc::new(move |name, data| seen.lock().unwrap().push((name, data))),
        );
        let bundle = f.sandbox.path().join("out.pathkeep");
        let error = if switch {
            vault_core::maintenance::switch_mode(
                &vault,
                None,
                ArchiveMode::Encrypted,
                Some(PASSWORD),
                Default::default(),
                |_| {},
            )
            .err()
            .unwrap()
        } else {
            vault_worker::export_app_data(&vault, bundle.clone(), EXPORT_PASSWORD).unwrap_err()
        };
        assert!(
            matches!(error.downcast_ref::<vault_store::Error>(),Some(vault_store::Error::RepairRequired(reason)) if matches!(reason.cause,vault_store::Error::RetainedBytes{revision,..} if revision==id)),
            "{error:#}"
        );
        assert_eq!(vault.conversation_availability().unwrap_err().role, Db::AgentHistoryEvidence);
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|(name, data)| *name == vault_worker::data_kinds::POLICY_EVENT
                    && data["changedKinds"] == serde_json::json!(["agent-history"]))
        );
        assert!(!bundle.exists());
    }
}

#[test]
fn surviving_revisions_reappear_after_fresh_start() {
    let f = Fixture::new(false);
    let (vault, id) = bad_digest(&f);
    assert!(vault.agent_history().unwrap().write_raw(id, &mut std::io::sink()).is_err());
    let plan = preview(&vault, None, None).unwrap();
    assert_eq!(plan.action, "startFresh");
    let back = plan.conversations.parse::<u64>().unwrap();
    assert!(back > 0);
    assert_eq!(back + plan.conversations_only_in_copy.as_ref().unwrap().parse::<u64>().unwrap(), 4);
    let installed = execute(&vault, None, None, &plan.token, Default::default()).unwrap();
    assert!(!sessions(&installed.vault.agent_history().unwrap()).is_empty());
}

#[test]
fn unlinked_worker_bytes_survive_in_marked_copy() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let vault = f.vault();
    vault.store().pause().unwrap().checkpoint().unwrap();
    let bytes = fs::read(evidence(&f)).unwrap();
    match fs::remove_file(evidence(&f)) {
        Ok(()) => {}
        Err(e) if cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33)) => {
            assert_eq!(fs::read(evidence(&f)).unwrap(), bytes);
            return;
        }
        Err(e) => panic!("unlinking fixture database: {e}"),
    }
    assert!(vault_core::maintenance::reconcile(&vault).unwrap().conversations_unreadable);
    assert!(vault.retry_conversations().is_err());
    let copy = vault_core::maintenance::manual_snapshot(&vault, true).unwrap();
    let name = std::path::Path::new(&copy.path)
        .join("held-conversations")
        .join(vault_layout::database_path(Db::AgentHistoryEvidence));
    assert_eq!(fs::read(name).unwrap(), bytes);
    assert!(!evidence(&f).exists());
    let plan = preview(&vault, None, None).unwrap();
    assert_eq!(plan.conversations, "4");
    let installed = execute(&vault, None, None, &plan.token, Default::default()).unwrap();
    assert_eq!(sessions(&installed.vault.agent_history().unwrap()).len(), 4);
    assert_eq!(
        fs::read(
            installed
                .previous_files
                .join("held-conversations")
                .join(vault_layout::database_path(Db::AgentHistoryEvidence))
        )
        .unwrap(),
        bytes
    );
}

#[cfg(unix)]
#[test]
fn denied_member_names_the_file_and_publishes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(false);
    let vault = broken(&f);
    let file = evidence(&f);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    let err = vault_core::maintenance::manual_snapshot(&vault, true).unwrap_err();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        err.to_string().contains("agent-history.db")
            && err.to_string().contains("nothing was changed"),
        "{err:#}"
    );
    assert!(vault_core::maintenance::list_snapshots(vault.paths()).unwrap().is_empty());
}

#[test]
fn historical_claude_transcripts_remain_raw_after_reparse_and_rebuild() {
    let f = Fixture::new(false);
    let vault = f.vault();
    let archive = vault.agent_history().unwrap();
    let root = f.sandbox.path().join("claude");
    let project = root.join("projects/p");
    fs::create_dir_all(&project).unwrap();
    for (name, id) in
        [("s.jsonl", "s"), ("s.jsonl.superseded-1", "old"), ("s.orphaned-1-old.jsonl", "orphan")]
    {
        let row = serde_json::json!({"type":"user","sessionId":id,"uuid":id,"message":{"role":"user","content":id}});
        fs::write(project.join(name), format!("{row}\n")).unwrap();
    }
    archive
        .backup(
            &[SourceRoot { root, harness: agent_history::Harness::ClaudeCode }],
            &Default::default(),
        )
        .unwrap();
    assert_eq!(sessions(&archive).len(), 1);
    for source in archive.list_sources(&Default::default()).unwrap().items {
        if !source.relative_path.ends_with("/s.jsonl") {
            let _ = archive.reparse_source(&source.id, &Default::default());
        }
    }
    assert_eq!(sessions(&archive).len(), 1);
    f.session.close_vault().unwrap();
    fs::remove_file(f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel))).unwrap();
    let vault = f.reopen(false);
    assert_eq!(preview(&vault, None, None).unwrap().conversations, "1");
}

#[test]
fn cancelled_superseded_and_abandoned_stages_cannot_execute() {
    let f = Fixture::new(false);
    let vault = broken(&f);
    let first = preview(&vault, None, None).unwrap();
    let second = preview(&vault, None, None).unwrap();
    assert_ne!(first.token, second.token);
    assert!(execute(&vault, None, None, &first.token, Default::default()).is_err());
    assert!(vault.cancel_conversation_repair_preview());
    assert!(fs::read_dir(&vault.paths().staging_dir).unwrap().next().is_none());
    assert!(execute(&vault, None, None, &second.token, Default::default()).is_err());
    let _last = preview(&vault, None, None).unwrap();
    f.session.close_vault().unwrap();
    assert!(fs::read_dir(&vault.paths().staging_dir).unwrap().next().is_none());
    let abandoned = vault.paths().staging_dir.join("maintenance-agent-preview-C4q1rE");
    fs::create_dir(&abandoned).unwrap();
    fs::write(abandoned.join("private"), b"abandoned trial").unwrap();
    let spool = vault.paths().staging_dir.join("maintenance-agent-preview-handle-12-4");
    fs::write(&spool, b"abandoned anchor").unwrap();
    f.reopen(false);
    assert!(!abandoned.exists());
    assert!(!spool.exists());
}

#[test]
fn repair_restore_and_rekey_bind_the_original_they_protected() {
    use vault_core::maintenance;
    for restore in [false, true] {
        let f = Fixture::new(false);
        let _corpus = Corpus::seed(&f);
        let good =
            f.vault().paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots).join("good");
        maintenance::snapshot(&f.vault(), &good).unwrap();
        let (vault, role) =
            if restore { (broken(&f), Db::Archive) } else { (f.vault(), Db::AgentHistoryEvidence) };
        let target = f.root.join(vault_layout::database_path(role));
        let path = target.clone();
        vault_core::fault_inject::once_at("native-install-before-prepare", move || {
            fs::write(path, b"newly restored original").unwrap()
        });
        let result = if restore {
            maintenance::restore_snapshot(&vault, &good, None, Default::default())
        } else {
            maintenance::switch_mode(
                &vault,
                None,
                ArchiveMode::Encrypted,
                Some(PASSWORD),
                Default::default(),
                |_| {},
            )
        };
        assert!(result.err().unwrap().to_string().contains("installation-source-changed"));
        assert_eq!(fs::read(target).unwrap(), b"newly restored original");
    }
}

#[test]
fn a_bad_export_destination_does_not_quarantine_conversations() {
    let f = Fixture::new(false);
    let parent = f.sandbox.path().join("regular-file");
    fs::write(&parent, b"untouched").unwrap();
    let vault = f.vault();
    let failed =
        vault_worker::export_app_data(&vault, parent.join("out.pathkeep"), EXPORT_PASSWORD)
            .unwrap_err();
    assert!(!format!("{failed:#}").contains("conversation-repair-required"));
    vault.conversation_availability().unwrap();
    assert_eq!(fs::read(parent).unwrap(), b"untouched");
}

#[cfg(unix)]
#[test]
fn sole_unlinked_generation_remains_listed_when_install_refuses() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let vault = f.vault();
    vault.store().pause().unwrap().checkpoint().unwrap();
    let bytes = fs::read(evidence(&f)).unwrap();
    fs::remove_file(evidence(&f)).unwrap();
    vault_core::maintenance::reconcile(&vault).unwrap();
    let plan = preview(&vault, None, None).unwrap();
    let path = evidence(&f);
    vault_core::fault_inject::once_at("conversation-repair-closed", move || {
        fs::write(path, b"new original").unwrap()
    });
    assert!(execute(&vault, None, None, &plan.token, Default::default()).is_err());
    let kept = std::path::Path::new(&plan.kept_as);
    assert!(
        vault_core::maintenance::list_snapshots(vault.paths())
            .unwrap()
            .iter()
            .any(|copy| std::path::Path::new(&copy.path) == kept)
    );
    assert_eq!(
        fs::read(
            kept.join(vault_layout::HELD_CONVERSATIONS)
                .join(vault_layout::database_path(Db::AgentHistoryEvidence))
        )
        .unwrap(),
        bytes
    );
    assert_eq!(fs::read(evidence(&f)).unwrap(), b"new original");
}

#[test]
fn damaged_identity_hint_cannot_hide_other_verified_conversations() {
    let f = Fixture::new(false);
    let (_, bad) = bad_digest(&f);
    f.session.close_vault().unwrap();
    let db = rusqlite::Connection::open(evidence(&f)).unwrap();
    db.execute(
        "UPDATE ah_source_conversations SET conversation_id='damaged hint' WHERE revision_id<>?1",
        [bad],
    )
    .unwrap();
    drop(db);
    let vault = f.reopen(false);
    assert!(vault.agent_history().unwrap().write_raw(bad, &mut std::io::sink()).is_err());
    let plan = preview(&vault, None, None).unwrap();
    assert!(plan.conversations.parse::<u64>().unwrap() >= 3);
    let installed = execute(&vault, None, None, &plan.token, Default::default()).unwrap();
    assert_eq!(
        sessions(&installed.vault.agent_history().unwrap()).len().to_string(),
        plan.conversations
    );
}
