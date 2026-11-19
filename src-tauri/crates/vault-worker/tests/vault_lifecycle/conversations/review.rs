//! Exercises repair races, offline publication and partial salvage with real archives.
//! Pair membership and layout admission have independent scenarios.
use super::*;
use agent_history::read_model::ReaderPublication;
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
        assert!(!vault.paths().app_root.join("archive/.pk-install-v2.json").exists());
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

#[cfg(unix)]
#[test]
fn round3_held_evidence_survives_quit_and_is_a_verified_repair_source() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let vault = f.vault();
    vault.store().pause().unwrap().checkpoint().unwrap();
    fs::remove_file(evidence(&f)).unwrap();
    vault_core::maintenance::reconcile(&vault).unwrap();
    assert!(vault.retry_conversations().is_err());
    f.session.close_vault().unwrap();
    let vault = f.reopen(false);
    let copies = vault_core::maintenance::list_snapshots(vault.paths()).unwrap();
    let copy = copies
        .iter()
        .find(|c| c.conversations_unreadable)
        .expect("no durable held evidence after quit");
    let source = std::path::Path::new(&copy.path);
    let plan = preview(&vault, Some(source), None).unwrap();
    assert_eq!(plan.conversations, "4");
    let installed = execute(&vault, Some(source), None, &plan.token, Default::default()).unwrap();
    assert_eq!(sessions(&installed.vault.agent_history().unwrap()).len(), 4);
}

#[cfg(unix)]
#[test]
fn round3_current_evidence_after_retry_includes_later_backup() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let v = f.vault();
    let good = f.sandbox.path().join("good");
    vault_core::maintenance::snapshot(&v, &good).unwrap();
    fs::remove_file(evidence(&f)).unwrap();
    vault_core::maintenance::reconcile(&v).unwrap();
    assert!(v.retry_conversations().is_err());
    fs::copy(good.join(vault_layout::database_path(Db::AgentHistoryEvidence)), evidence(&f))
        .unwrap();
    v.retry_conversations().unwrap();
    let root = f.sandbox.path().join("late");
    let path = root.join("projects/p/late.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, r#"{"type":"user","sessionId":"late","uuid":"u","cwd":"/synthetic/late","timestamp":"2026-11-19T01:00:00Z","message":{"role":"user","content":"later backup lateuniqueneedle"}}
"#).unwrap();
    v.agent_history()
        .unwrap()
        .backup(
            &[SourceRoot { root, harness: agent_history::Harness::ClaudeCode }],
            &Default::default(),
        )
        .unwrap();
    let search = agent_history::search::SearchQuery {
        text: "lateuniqueneedle".into(),
        harness: None,
        project_path: None,
        limit: 10,
    };
    let original = v.agent_history().unwrap();
    original.synchronize_search().unwrap();
    let saved = original.search(&search).unwrap();
    assert_eq!(saved.total, 1);
    let saved_id = saved.items[0].session_id.clone();
    drop(original);
    fs::remove_file(path).unwrap();
    fs::remove_file(f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel))).unwrap();
    vault_core::maintenance::reconcile(&v).unwrap();
    let plan = preview(&v, None, None).unwrap();
    assert_eq!(plan.conversations, "5");
    let repaired = execute(&v, None, None, &plan.token, Default::default()).unwrap();
    let archive = repaired.vault.agent_history().unwrap();
    archive.synchronize_search().unwrap();
    let found = archive.search(&search).unwrap();
    assert_eq!(found.total, 1);
    assert_eq!(found.items[0].session_id, saved_id);
    let late = archive
        .list_sessions(&Default::default())
        .unwrap()
        .items
        .into_iter()
        .find(|s| s.id == saved_id)
        .expect("the later backup was lost");
    let mut raw = Vec::new();
    archive.write_raw(late.revision_id, &mut raw).unwrap();
    assert!(String::from_utf8(raw).unwrap().contains("later backup"));
    let relative = vault_layout::database_path(Db::AgentHistoryEvidence);
    assert_eq!(
        fs::read(
            repaired
                .previous_files
                .join(vault_layout::HELD_CONVERSATIONS)
                .join("older-1")
                .join(relative)
        )
        .unwrap(),
        fs::read(good.join(relative)).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn round3_access_retry_keeps_active_unticketed_data() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let v = f.vault();
    fs::set_permissions(evidence(&f), fs::Permissions::from_mode(0o000)).unwrap();
    assert!(vault_core::maintenance::reconcile(&v).unwrap().conversations_unreadable);
    let listed = serde_json::to_value(vault_worker::data_kinds::list(&f.session).unwrap()).unwrap();
    fs::set_permissions(evidence(&f), fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(listed["kinds"][1]["failure"]["reason"], "access-denied");
    v.retry_conversations().unwrap();
    assert_eq!(sessions(&v.agent_history().unwrap()).len(), 4);
}

#[test]
fn round3_partial_latest_optional_hints_and_mirror_only_marks() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let v = f.vault();
    let archive = v.agent_history().unwrap();
    let source_id =
        sessions(&archive)["claude-code"]["sources"][0]["source_id"].as_str().unwrap().to_owned();
    let source = archive.source(&source_id).unwrap();
    let previous = archive.latest_revision(&source_id).unwrap().unwrap().id;
    let mut bytes = Vec::new();
    archive.write_raw(previous, &mut bytes).unwrap();
    let root = std::path::PathBuf::from(&source.root);
    let path = root.join(&source.relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let added = serde_json::json!({"type":"user","sessionId":fixtures::SHARED_ID,"uuid":"new-message","message":{"role":"user","content":"only-in-the-last-revision"}});
    bytes.extend(format!("{added}\n").bytes());
    fs::write(&path, bytes).unwrap();
    archive
        .backup(
            &[SourceRoot { root: root.clone(), harness: agent_history::Harness::ClaudeCode }],
            &Default::default(),
        )
        .unwrap();
    fs::remove_dir_all(&root).unwrap();
    let rev = archive.latest_revision(&source_id).unwrap().unwrap().id;
    assert_ne!(rev, previous);
    f.session.close_vault().unwrap();
    let db = rusqlite::Connection::open(evidence(&f)).unwrap();
    db.execute("UPDATE ah_revisions SET digest=lower(hex(zeroblob(32))) WHERE id=?1", [rev])
        .unwrap();
    db.execute("INSERT INTO ah_parse_reports SELECT revision_id,'bad:record=999999999:decoded=1','{}' FROM ah_parse_reports WHERE revision_id<>?1 LIMIT 1",[rev]).unwrap();
    drop(db);
    let model = rusqlite::Connection::open(
        f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel)),
    )
    .unwrap();
    model
        .execute(
            "INSERT INTO ah_user_state VALUES('unrelated-conversation',1,1,NULL,'foreign')",
            [],
        )
        .unwrap();
    model.execute("INSERT OR REPLACE INTO ah_user_state SELECT conversation_id,1,1,NULL,'spurious mirror' FROM ah_sessions WHERE harness='pi'",[]).unwrap();
    drop(model);
    let v = f.reopen(false);
    assert!(v.agent_history().unwrap().write_raw(rev, &mut std::io::sink()).is_err());
    let start = std::time::Instant::now();
    let plan = preview(&v, None, None).unwrap();
    let elapsed = start.elapsed();
    let fields = serde_json::to_value(&plan).unwrap();
    assert_eq!(fields["conversationsRecoveredPartially"], "1");
    assert_eq!(fields["conversationsRecoveredWhole"], "3");
    assert_eq!(fields["conversationsOnlyInCopy"], "0");
    assert!(fields["skippedOptionalRecords"].as_str().unwrap().parse::<usize>().unwrap() > 0);
    let out = execute(&v, None, None, &plan.token, Default::default()).unwrap();
    let marks: i64 = out
        .vault
        .store()
        .read(Db::AgentHistoryEvidence, |s| {
            s.query_one(
                "SELECT count(*) FROM ah_user_state WHERE conversation_id='unrelated-conversation'",
                (),
                |r| Ok(r.get(0)?),
            )
        })
        .unwrap();
    let result = sessions(&out.vault.agent_history().unwrap());
    assert_eq!(result["pi"]["starred"], false);
    assert_eq!(result["claude-code"]["revision_id"], previous.to_string());
    assert_eq!(marks, 0);
    eprintln!("salvage preview {elapsed:?}");
}

#[test]
fn round3_immutable_verification_precedes_lease() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let big = f.sandbox.path().join("large-claude");
    fs::create_dir_all(big.join("uploads")).unwrap();
    let mut random = 0xa3147259u32;
    let bytes = (0..16 * 1024 * 1024)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as u8
        })
        .collect::<Vec<_>>();
    fs::write(big.join("uploads/retained.bin"), bytes).unwrap();
    fs::create_dir_all(big.join("projects/measure")).unwrap();
    {
        use std::io::Write;
        let mut transcript = std::io::BufWriter::new(
            fs::File::create(big.join("projects/measure/s.jsonl")).unwrap(),
        );
        let body = "recoverable message ".repeat(432);
        for index in 0..1024 {
            let row = serde_json::json!({"type":"user","sessionId":"large-salvage","uuid":format!("m{index}"),"message":{"role":"user","content":body}});
            writeln!(transcript, "{row}").unwrap();
        }
    }
    f.vault()
        .agent_history()
        .unwrap()
        .backup(
            &[SourceRoot { root: big.clone(), harness: agent_history::Harness::ClaudeCode }],
            &Default::default(),
        )
        .unwrap();
    fs::remove_dir_all(big).unwrap();
    let raw:i64=f.vault().store().read(Db::AgentHistoryEvidence,|s|s.query_one("SELECT sum(byte_len) FROM ah_revisions WHERE state='complete' AND bytes_retained=1",(),|r|Ok(r.get(0)?))).unwrap();
    assert!(raw >= 16 * 1024 * 1024);
    let v = f.vault();
    let (pi, other) = damage_pi_reader(&v);
    assert_reader_isolated(&v, &pi, &other);
    let (live, parked, healthy) = (v.clone(), pi.clone(), other.clone());
    vault_core::fault_inject::once_at("conversation-repair-preparing", move || {
        assert_reader_isolated(&live, &parked, &healthy);
    });
    let prepare = std::time::Instant::now();
    let plan = preview(&v, None, None).unwrap();
    let prepare = prepare.elapsed();
    assert_eq!(plan.action, "rebuild");
    assert_eq!(plan.conversations, "1");
    assert_reader_isolated(&v, &pi, &other);
    let verified = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = verified.clone();
    vault_core::fault_inject::once_at("conversation-repair-verified", move || flag.set(true));
    let clock = std::rc::Rc::new(std::cell::Cell::new(None));
    let begin = clock.clone();
    vault_core::fault_inject::once_at("conversation-repair-lease-start", move || {
        assert!(verified.get(), "exclusive lease acquired before stage verification");
        begin.set(Some(std::time::Instant::now()));
    });
    vault_core::fault_inject::once_at("conversation-repair-lease-end", move || {
        eprintln!(
            "retained {raw} bytes; reader repair preview {prepare:?}, lease {:?}",
            clock.get().unwrap().elapsed()
        )
    });
    vault_worker::data_kinds::repair::execute(
        &f.session,
        serde_json::from_value(serde_json::json!({"previewToken":plan.token})).unwrap(),
        |_| {},
    )
    .unwrap();
    assert_reader_repaired(&f.vault(), &pi, &other);
}

#[cfg(unix)]
#[test]
fn round3_mirror_marks_need_a_matching_model_and_a_recovered_identity() {
    for rejected in [false, true] {
        let f = Fixture::new(false);
        let _corpus = Corpus::seed(&f);
        let id: String = f
            .vault()
            .store()
            .read(Db::AgentHistoryReadModel, |s| {
                s.query_one(
                    "SELECT conversation_id FROM ah_sessions WHERE harness='pi' LIMIT 1",
                    (),
                    |r| Ok(r.get(0)?),
                )
            })
            .unwrap();
        f.session.close_vault().unwrap();
        let db = rusqlite::Connection::open(evidence(&f)).unwrap();
        db.execute("DROP TABLE ah_user_state", []).unwrap();
        drop(db);
        let path = f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel));
        let model = rusqlite::Connection::open(path).unwrap();
        model
            .execute(
                "INSERT OR REPLACE INTO ah_user_state VALUES(?1,1,0,NULL,'readable mirror')",
                [&id],
            )
            .unwrap();
        if rejected {
            model.execute("UPDATE ah_pair_seal SET ticket=lower(hex(randomblob(32)))", []).unwrap();
        }
        drop(model);
        let v = f.reopen(false);
        if !rejected {
            v.store().pause().unwrap().checkpoint().unwrap();
            fs::remove_file(evidence(&f)).unwrap();
            vault_core::maintenance::reconcile(&v).unwrap();
        }
        let plan = preview(&v, None, None).unwrap();
        let out = execute(&v, None, None, &plan.token, Default::default()).unwrap();
        let pi = sessions(&out.vault.agent_history().unwrap())["pi"].clone();
        assert_eq!(pi["starred"], !rejected, "mirror rejected={rejected}");
    }
}

#[cfg(unix)]
#[test]
fn round3_execute_names_newly_inaccessible_member() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(false);
    let v = broken(&f);
    let plan = preview(&v, None, None).unwrap();
    let path = evidence(&f);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let err = execute(&v, None, None, &plan.token, Default::default()).err().unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        format!("{err:#}").contains("conversation-preservation-failed")
            && format!("{err:#}").contains("agent-history.db"),
        "{err:#}"
    );
    assert_eq!(fs::read(path).unwrap(), b"generation A");
}

#[test]
fn round3_missing_public_inventory_uses_evidence_and_reports_unknown_when_neither_works() {
    for unknown in [false, true] {
        let f = Fixture::new(false);
        let (_, bad) = bad_digest(&f);
        f.session.close_vault().unwrap();
        let model = rusqlite::Connection::open(
            f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel)),
        )
        .unwrap();
        model.execute("DROP TABLE ah_sessions", []).unwrap();
        drop(model);
        if unknown {
            rusqlite::Connection::open(evidence(&f))
                .unwrap()
                .execute("DROP TABLE ah_source_conversations", [])
                .unwrap();
        }
        let v = f.reopen(false);
        if let Ok(archive) = v.agent_history() {
            archive.write_raw(bad, &mut std::io::sink()).unwrap_err();
        }
        let plan = preview(&v, None, None).unwrap();
        assert!(plan.conversations.parse::<usize>().unwrap() > 0);
        assert_eq!(plan.conversations_only_in_copy.is_none(), unknown);
    }
}

#[cfg(unix)]
#[test]
fn round3_close_refused_until_the_only_handle_is_durable() {
    let f = Fixture::new(true);
    let _corpus = Corpus::seed(&f);
    let v = f.vault();
    v.store().pause().unwrap().checkpoint().unwrap();
    let copies = v.paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots);
    if copies.exists() {
        fs::remove_dir(&copies).unwrap();
    }
    fs::create_dir_all(copies.parent().unwrap()).unwrap();
    fs::write(&copies, b"blocks publication").unwrap();
    fs::remove_file(evidence(&f)).unwrap();
    assert!(f.session.close_vault().is_err());
    assert!(f.session.current_vault().is_some());
    fs::remove_file(copies).unwrap();
    f.session.close_vault().unwrap();
    let vault = f.reopen(true);
    let listed = vault_core::maintenance::list_snapshots(vault.paths()).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].encrypted);
    assert_eq!(
        preview(&vault, Some(std::path::Path::new(&listed[0].path)), Some(PASSWORD))
            .unwrap()
            .conversations,
        "4"
    );
}

// Damage a published reader version while its retained transcript remains intact.
fn damage_pi_reader(v: &Vault) -> (String, Vec<ReaderPublication>) {
    let archive = v.agent_history().unwrap();
    let ids = v
        .store()
        .read(Db::AgentHistoryReadModel, |s| {
            s.query(
                "SELECT harness,conversation_id FROM ah_sessions WHERE conversation_current=1",
                (),
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
            )
        })
        .unwrap();
    let pi = ids.iter().find(|(tool, _)| tool == "pi").unwrap().1.clone();
    let other = ids
        .into_iter()
        .filter(|(_, id)| id != &pi)
        .map(|(_, id)| archive.reader_header(&id).unwrap())
        .collect();
    let damaged = pi.clone();
    v.store().write(Db::AgentHistoryReadModel,vault_store::Priority::Interactive,move |s| {
        assert_eq!(s.execute("UPDATE ah_reader_version SET digest='invalid' WHERE token=(SELECT version FROM ah_reader_member m JOIN ah_reader_head h ON h.publication=m.publication WHERE h.conversation=?1 AND m.source=h.current_source)",[damaged.as_str()])?,1);
        s.execute("INSERT OR REPLACE INTO ah_reader_dirty VALUES(?1,lower(hex(randomblob(16))))",[damaged])?;
        Ok(())
    }).unwrap();
    (pi, other)
}

fn assert_reader_isolated(v: &Vault, pi: &str, other: &[ReaderPublication]) {
    v.conversation_availability().unwrap();
    let archive = v.agent_history().unwrap();
    assert_eq!(archive.reader_header(pi).unwrap_err().code(), "reader_rebuild_required");
    for healthy in other {
        assert_eq!(
            archive.reader_header(&healthy.conversation_id).unwrap().publication,
            healthy.publication
        );
    }
}

fn assert_reader_repaired(v: &Vault, pi: &str, other: &[ReaderPublication]) {
    v.conversation_availability().unwrap();
    let archive = v.agent_history().unwrap();
    let head = archive.reader_header(pi).unwrap();
    let rows = archive.reader_window(&head, None, 0, 4, &Default::default()).unwrap();
    assert!(rows.iter().flat_map(|row| &row.parts).any(|part| part.text == "pianswerneedle"));
    for healthy in other {
        assert_eq!(
            archive.reader_header(&healthy.conversation_id).unwrap().publication,
            healthy.publication
        );
    }
    assert!(
        preview(v, None, None).unwrap_err().to_string().contains("conversation-repair-healthy")
    );
}

#[test]
fn reader_repair_recovers_a_parked_conversation_and_keeps_healthy_publications() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let corpus = Corpus::seed(&f);
        assert!(
            preview(&f.vault(), None, None)
                .unwrap_err()
                .to_string()
                .contains("conversation-repair-healthy")
        );
        let (pi, other) = damage_pi_reader(&f.vault());
        f.session.close_vault().unwrap();
        let v = f.reopen(encrypted);
        assert_reader_isolated(&v, &pi, &other);
        let parked = v
            .store()
            .read(Db::AgentHistoryReadModel, |s| {
                s.query_one("SELECT count(*) FROM ah_reader_parked", (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap();
        assert_eq!(parked, 1);
        let kinds =
            serde_json::to_value(vault_worker::data_kinds::list(&f.session).unwrap()).unwrap();
        assert_eq!(kinds["kinds"][1]["availability"], "available");
        let superseded = preview(&v, None, None).unwrap();
        v.store()
            .write(Db::AgentHistoryReadModel, vault_store::Priority::Interactive, |s| {
                s.execute("UPDATE ah_reader_parked SET ticket=lower(hex(randomblob(16)))", ())
            })
            .unwrap();
        let stale = execute(&v, None, None, &superseded.token, Default::default()).err().unwrap();
        assert!(stale.to_string().contains("conversation-repair-preview-stale"), "{stale:#}");
        assert_reader_isolated(&v, &pi, &other);
        let plan = preview(&v, None, None).unwrap();
        assert_eq!(plan.action, "rebuild");
        assert_eq!(plan.conversations, "1");
        assert_eq!(plan.conversations_recovered_whole.as_deref(), Some("1"));
        assert_reader_isolated(&v, &pi, &other);
        let installed = execute(&v, None, None, &plan.token, Default::default()).unwrap();
        assert_reader_repaired(&installed.vault, &pi, &other);
        corpus.check(&installed.vault, encrypted.then_some(PASSWORD));
    }
}

#[cfg(unix)]
#[test]
fn idle_owner_waits_for_the_unlocked_shell_before_preserving() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let v = f.vault();
    v.store().pause().unwrap().checkpoint().unwrap();
    let original = fs::read(evidence(&f)).unwrap();
    fs::remove_file(evidence(&f)).unwrap();

    // Two intervals of the former one-second watcher, without a window event or SQL job.
    std::thread::sleep(std::time::Duration::from_millis(2200));
    let copies = || vault_core::maintenance::list_snapshots(v.paths()).unwrap();
    assert!(!copies().iter().any(|s| s.source_op == "heldGeneration"));
    vault_worker::data_kinds::list(&f.session).unwrap();
    let saved = copies().into_iter().find(|s| s.source_op == "heldGeneration").unwrap();
    let path = std::path::Path::new(&saved.path)
        .join(vault_layout::HELD_CONVERSATIONS)
        .join(vault_layout::database_path(Db::AgentHistoryEvidence));
    assert_eq!(fs::read(path).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn conversation_write_preserves_unlinked_sibling_before_running() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let v = f.vault();
    v.store().pause().unwrap().checkpoint().unwrap();
    let original = fs::read(evidence(&f)).unwrap();
    fs::remove_file(evidence(&f)).unwrap();

    let result = v.store().write(
        Db::AgentHistoryReadModel,
        vault_store::Priority::Interactive,
        |_| -> vault_store::Result<()> { panic!("write ran against a displaced group") },
    );
    assert!(
        matches!(result, Err(vault_store::Error::Open(reason)) if reason.role == Db::AgentHistoryEvidence)
    );
    let copies = vault_core::maintenance::list_snapshots(v.paths()).unwrap();
    let saved = copies.iter().find(|s| s.source_op == "heldGeneration").unwrap();
    let path = std::path::Path::new(&saved.path)
        .join(vault_layout::HELD_CONVERSATIONS)
        .join(vault_layout::database_path(Db::AgentHistoryEvidence));
    assert_eq!(fs::read(path).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn round3_bulk_prune_cannot_sweep_a_newly_published_held_copy() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    let vault = f.vault();
    vault.store().pause().unwrap().checkpoint().unwrap();
    let paths = vault.paths();
    vault_core::maintenance::prune(
        &vault,
        &vault_core::RetentionPruneRequest {
            bucket_ids: vec!["snapshots".into(), "staging".into()],
            ..Default::default()
        },
        || {
            let reviewed = vault_core::maintenance::list_snapshots(paths)?;
            fs::remove_file(evidence(&f))?;
            vault.check_available_files()?;
            Ok(reviewed)
        },
        &Default::default(),
    )
    .unwrap();
    let copies = vault_core::maintenance::list_snapshots(paths).unwrap();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].source_op, "heldGeneration");
}

#[test]
fn identical_pi_content_is_whole_when_either_physical_revision_is_damaged() {
    for damage_current in [false, true] {
        let f = Fixture::new(false);
        let v = f.vault();
        let archive = v.agent_history().unwrap();
        let roots = ["first", "second"].map(|name| SourceRoot {
            root: f.sandbox.path().join(name),
            harness: agent_history::Harness::Pi,
        });
        let first = fixtures::pi(
            &roots[0].root,
            "/synthetic/pi",
            fixtures::SHARED_ID,
            "duplicatecontentneedle",
        );
        let bytes = fs::read(&first).unwrap();
        let second = roots[1].root.join(first.strip_prefix(&roots[0].root).unwrap());
        fs::create_dir_all(second.parent().unwrap()).unwrap();
        fs::write(&second, &bytes).unwrap();
        archive.backup(&roots, &Default::default()).unwrap();
        let rows = archive.list_sessions(&Default::default()).unwrap().items;
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.sources.len(), 2);
        let source = row.sources.iter().find(|s| s.current == damage_current).unwrap();
        let bad = archive.latest_revision(&source.source_id).unwrap().unwrap().id;
        f.session.close_vault().unwrap();
        rusqlite::Connection::open(evidence(&f))
            .unwrap()
            .execute("UPDATE ah_revisions SET digest=lower(hex(zeroblob(32))) WHERE id=?1", [bad])
            .unwrap();
        let v = f.reopen(false);
        assert!(v.agent_history().unwrap().write_raw(bad, &mut std::io::sink()).is_err());
        let plan = preview(&v, None, None).unwrap();
        assert_eq!(
            plan.conversations_recovered_whole.as_deref(),
            Some("1"),
            "damaged current={damage_current}"
        );
        assert_eq!(plan.conversations_recovered_partially.as_deref(), Some("0"));
        assert_eq!(plan.conversations_only_in_copy.as_deref(), Some("0"));
        let installed = execute(&v, None, None, &plan.token, Default::default()).unwrap();
        let repaired = installed.vault.agent_history().unwrap();
        let after = repaired.list_sessions(&Default::default()).unwrap().items;
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, row.id);
        let mut recovered = Vec::new();
        repaired.write_raw(after[0].revision_id, &mut recovered).unwrap();
        assert_eq!(bytes, recovered);
    }
}
