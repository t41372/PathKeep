//! Exercises the repair preview and its irreversible boundary on synthetic archives.
//! Corpus and browser fixtures retain the independent expected data.

#[path = "pre_pair_copy.rs"]
mod pre_pair_copy;
use super::*;
use vault_core::maintenance::{preview_conversation_repair, repair_conversations};
use vault_store::Db;

#[test]
fn repair_from_copy_and_start_fresh_preserve_the_displaced_data() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let corpus = Corpus::seed(&f);
        let copies = f.vault().paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots);
        let good = copies.join("rekey-good");
        vault_core::maintenance::snapshot(&f.vault(), &good).unwrap();
        // A different live key/mode from the source copy.
        f.rekey(
            if encrypted { ArchiveMode::Plaintext } else { ArchiveMode::Encrypted },
            encrypted.then_some(PASSWORD),
            (!encrypted).then_some(NEW_PASSWORD),
        );
        let vault = f.vault();
        let live = f.root.join(vault_layout::database_path(Db::AgentHistoryEvidence));
        let paths = vault.paths().clone();
        f.session.close_vault().unwrap();
        fs::write(&live, b"damaged evidence").unwrap();
        let config = vault_core::load_config(&paths).unwrap();
        let vault = if encrypted {
            Vault::open(paths.clone(), config, None).unwrap()
        } else {
            Vault::open_password(paths.clone(), config, NEW_PASSWORD, None).unwrap().0
        };
        let bad = copies.join("during-damage");
        vault_core::maintenance::snapshot(&vault, &bad).unwrap();
        assert!(preview_conversation_repair(&vault, Some(&bad), None).is_err());
        let preview =
            preview_conversation_repair(&vault, Some(&good), encrypted.then_some(PASSWORD))
                .unwrap();
        assert_eq!(preview.action, "fromCopy");
        assert_eq!(preview.conversations, "4");
        let installed = repair_conversations(
            &vault,
            Some(&good),
            encrypted.then_some(PASSWORD),
            &preview.token,
            Default::default(),
        )
        .unwrap();
        // The source can be an intentionally retained plaintext historical snapshot.
        // Free that exact copy through retention before auditing the encrypted live root.
        vault_core::maintenance::prune(
            &installed.vault,
            &vault_core::RetentionPruneRequest {
                copies: vec![good.display().to_string()],
                ..Default::default()
            },
            || vault_core::maintenance::list_snapshots(&paths),
            &Default::default(),
        )
        .unwrap();
        corpus.check(&installed.vault, (!encrypted).then_some(NEW_PASSWORD));
        assert_eq!(
            fs::read(
                installed
                    .previous_files
                    .join(vault_layout::database_path(Db::AgentHistoryEvidence))
            )
            .unwrap(),
            b"damaged evidence"
        );
        assert!(
            vault_core::maintenance::list_snapshots(&paths)
                .unwrap()
                .iter()
                .any(|c| c.path == installed.previous_files.display().to_string()
                    && c.conversations_unreadable
                    && c.size_bytes > 0)
        );
        let retention = vault_core::maintenance::preview_retention(
            &paths,
            &vault_core::maintenance::list_snapshots(&paths).unwrap(),
        )
        .unwrap();
        let row = retention
            .copies
            .iter()
            .find(|c| c.path == installed.previous_files.display().to_string())
            .unwrap();
        assert!(row.bytes > 0 && row.conversations_unreadable);
        let used_before =
            vault_worker::dashboard_snapshot(&installed.vault).unwrap().storage.snapshot_bytes;
        let freed = vault_core::maintenance::prune(
            &installed.vault,
            &vault_core::RetentionPruneRequest {
                copies: vec![row.path.clone()],
                ..Default::default()
            },
            || vault_core::maintenance::list_snapshots(&paths),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(freed.deleted_bytes, row.bytes);
        let used_after =
            vault_worker::dashboard_snapshot(&installed.vault).unwrap().storage.snapshot_bytes;
        assert_eq!(used_before - used_after, row.bytes);
        assert!(!installed.previous_files.exists());
        installed.vault.close().unwrap();
        fs::write(&live, b"unreadable again").unwrap();
        let config = vault_core::load_config(&paths).unwrap();
        let vault = Vault::open_owned(paths, config, installed.data_key.as_ref(), None).unwrap();
        let fresh = preview_conversation_repair(&vault, None, None).unwrap();
        assert_eq!(fresh.action, "startFresh");
        assert!(
            repair_conversations(&vault, None, None, &preview.token, Default::default()).is_err()
        );
        let installed =
            repair_conversations(&vault, None, None, &fresh.token, Default::default()).unwrap();
        assert!(sessions(&installed.vault.agent_history().unwrap()).is_empty());
        assert_eq!(
            fs::read(
                installed
                    .previous_files
                    .join(vault_layout::database_path(Db::AgentHistoryEvidence))
            )
            .unwrap(),
            b"unreadable again"
        );
    }
}

#[test]
fn intact_evidence_allows_only_lossless_rebuild() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let corpus = Corpus::seed(&f);
        let grant = f.vault().config().capture_policy.clone();
        let older = f
            .vault()
            .paths()
            .artifact_path(vault_layout::ArtifactId::NativeSnapshots)
            .join("manual-older");
        vault_core::maintenance::snapshot(&f.vault(), &older).unwrap();
        f.session.close_vault().unwrap();
        fs::write(
            f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel)),
            b"damaged projection",
        )
        .unwrap();
        let vault = f.reopen(encrypted);
        let refused =
            preview_conversation_repair(&vault, Some(&older), encrypted.then_some(PASSWORD))
                .unwrap_err();
        assert!(refused.to_string().contains("conversation-evidence-readable"));
        let preview = preview_conversation_repair(&vault, None, None).unwrap();
        assert_eq!(preview.action, "rebuild");
        let repaired =
            repair_conversations(&vault, None, None, &preview.token, Default::default()).unwrap();
        assert_eq!(repaired.vault.config().capture_policy, grant);
        corpus.check_rebuilt(&repaired.vault, encrypted.then_some(PASSWORD));
    }
}

#[test]
fn process_exit_recovers_copy_repair_and_fresh_start_at_each_decision() {
    const CHILD: &str = "PATHKEEP_CONVERSATION_REPAIR_CUT";
    if let Ok(point) = std::env::var(CHILD) {
        let paths = vault_core::project_paths().unwrap();
        let config = vault_core::load_config(&paths).unwrap();
        let encrypted = config.archive_mode == ArchiveMode::Encrypted;
        let vault = if encrypted {
            Vault::open_password(paths.clone(), config, PASSWORD, None).unwrap().0
        } else {
            Vault::open(paths.clone(), config, None).unwrap()
        };
        let good = paths.artifact_path(vault_layout::ArtifactId::NativeSnapshots).join("source");
        let source = (std::env::var("PATHKEEP_REPAIR_FROM_COPY").unwrap() == "true")
            .then_some(good.as_path());
        let plan =
            preview_conversation_repair(&vault, source, encrypted.then_some(PASSWORD)).unwrap();
        vault_core::fault_inject::exit_process_at_checkpoint_hit(
            &point,
            std::env::var("PATHKEEP_REPAIR_HIT").unwrap().parse().unwrap(),
        );
        repair_conversations(
            &vault,
            source,
            encrypted.then_some(PASSWORD),
            &plan.token,
            Default::default(),
        )
        .unwrap();
        return;
    }
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let corpus = Corpus::seed(&f);
        let copies = f.vault().paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots);
        vault_core::maintenance::snapshot(&f.vault(), &copies.join("source")).unwrap();
        f.session.close_vault().unwrap();
        let evidence = vault_layout::database_path(Db::AgentHistoryEvidence);
        fs::write(f.root.join(evidence), b"original damaged bytes").unwrap();
        for from_copy in [false, true] {
            for (point, committed, repeated) in [
                ("conversation-repair-verified", false, false),
                ("conversation-repair-safety-sealed", false, false),
                ("conversation-repair-closed", false, false),
                ("native-install-prepared", false, false),
                ("native-install-preserved", false, true),
                ("native-install-installed", false, true),
                ("native-install-publish-saved", true, false),
                ("native-install-published", true, false),
                ("conversation-repair-installed", true, false),
            ] {
                let mut hit = 1;
                loop {
                    let target = tempfile::tempdir_in(f.sandbox.path()).unwrap();
                    for name in expected_root::files_under(&f.root).unwrap() {
                        let dest = target.path().join(&name);
                        fs::create_dir_all(dest.parent().unwrap()).unwrap();
                        fs::copy(f.root.join(name), dest).unwrap();
                    }
                    let out=std::process::Command::new(std::env::current_exe().unwrap())
                        .args(["--exact","conversations::repair::process_exit_recovers_copy_repair_and_fresh_start_at_each_decision"])
                        .env(CHILD,point).env("PATHKEEP_REPAIR_FROM_COPY",from_copy.to_string())
                        .env("PATHKEEP_REPAIR_HIT",hit.to_string()).env("CHB_PROJECT_ROOT",target.path())
                        .output().unwrap();
                    if repeated && out.status.success() {
                        assert!(hit > 4, "missing install cuts: {point}");
                        break;
                    }
                    assert_eq!(
                        out.status.code(),
                        Some(86),
                        "{point} {hit}: {}",
                        String::from_utf8_lossy(&out.stdout)
                    );
                    let paths = vault_core::project_paths_with_root(target.path());
                    let owner = Arc::new(vault_store::OwnerLock::acquire(target.path()).unwrap());
                    vault_core::maintenance::recover_native_maintenance(&paths).unwrap();
                    let config = vault_core::load_config(&paths).unwrap();
                    let opened = if encrypted {
                        Vault::open_password(paths.clone(), config, PASSWORD, Some(owner))
                            .unwrap()
                            .0
                    } else {
                        Vault::open_owned(paths.clone(), config, None, Some(owner)).unwrap()
                    };
                    let copies = vault_core::maintenance::list_snapshots(&paths).unwrap();
                    if committed {
                        let prior = copies.iter().find(|c| c.conversations_unreadable).unwrap();
                        assert_eq!(
                            fs::read(std::path::Path::new(&prior.path).join(evidence)).unwrap(),
                            b"original damaged bytes"
                        );
                        opened.conversation_availability().unwrap();
                        if from_copy {
                            corpus.check(&opened, encrypted.then_some(PASSWORD));
                        } else {
                            assert!(sessions(&opened.agent_history().unwrap()).is_empty());
                        }
                    } else {
                        assert!(opened.conversation_availability().is_err());
                        assert_eq!(
                            fs::read(target.path().join(evidence)).unwrap(),
                            b"original damaged bytes"
                        );
                        assert_eq!(copies.len(), 1);
                    }
                    opened.close().unwrap();
                    if !repeated {
                        break;
                    }
                    hit += 1;
                    assert!(hit <= 16, "unbounded entry list");
                }
            }
        }
    }
}

#[test]
fn screens_can_preview_protect_and_repair_an_unavailable_kind() {
    use serde_json::{from_value, json, to_value};
    use vault_worker::data_kinds::repair;
    let f = Fixture::new(false);
    let preview_req = |v| from_value(v).unwrap();
    assert_eq!(
        repair::preview(&f.session, preview_req(json!({}))).unwrap_err().to_string(),
        "conversation-repair-healthy"
    );
    let _corpus = Corpus::seed(&f);
    f.session.close_vault().unwrap();
    fs::write(f.root.join(vault_layout::database_path(Db::AgentHistoryEvidence)), b"bad").unwrap();
    f.reopen(false);
    let old_policy = f.vault().config().capture_policy.clone();
    assert!(old_policy.selection_ref.is_some());
    let manual = repair::preview_safety_copy(&f.session).unwrap();
    assert!(manual.conversations_unreadable);
    let save = json!({"previewToken":manual.preview_token});
    let copy = repair::create_safety_copy(&f.session, from_value(save.clone()).unwrap()).unwrap();
    assert!(copy.conversations_unreadable);
    assert_eq!(copy.source_op, "manual");
    let reject =
        repair::preview(&f.session, preview_req(json!({"copyPath":copy.path}))).unwrap_err();
    assert!(reject.to_string().contains("conversation-copy-no-verified-evidence"));
    let preview = repair::preview(&f.session, preview_req(json!({}))).unwrap();
    assert_eq!(preview.action, "startFresh");
    assert!(
        preview.next_backup_recaptures_tool_files && preview.archive_only_conversations_unreadable
    );
    let request = json!({"previewToken":preview.token});
    let mut event = None;
    let report = repair::execute(&f.session, from_value(request.clone()).unwrap(), |e| {
        event = Some(to_value(e).unwrap())
    })
    .unwrap();
    assert_eq!(report.kept_copy_path, preview.kept_as);
    assert_eq!(report.availability, "available");
    assert_eq!(event.as_ref().unwrap()["changedKinds"], json!(["agent-history"]));
    let policy = f.vault().config().capture_policy.clone();
    assert_eq!(policy.agent_history, old_policy.agent_history);
    assert!(policy.selection_ref.is_none());
    assert!(policy.pending_first_save.is_none());
    let artifact =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts/vault-lifecycle");
    fs::create_dir_all(&artifact).unwrap();
    fs::write(artifact.join("repair-contract.json"),serde_json::to_vec_pretty(&json!({
        "preview_archive_safety_copy":{"request":{},"response":manual},
        "create_archive_safety_copy":{"request":save,"response":copy},
        "preview_agent_history_repair":{"request":{},"response":preview},
        "repair_agent_history":{"request":request,"response":report,"event":{"name":vault_worker::data_kinds::POLICY_EVENT,"payload":event}}
    })).unwrap()).unwrap();
}

#[test]
fn encrypted_copy_requires_its_own_password_and_keeps_the_live_key() {
    let f = Fixture::new(true);
    let corpus = Corpus::seed(&f);
    let paths = f.vault().paths().clone();
    let original_key = Vault::unlock_data_key(&paths, PASSWORD).unwrap();
    let good =
        paths.artifact_path(vault_layout::ArtifactId::NativeSnapshots).join("manual-original");
    vault_core::maintenance::snapshot(&f.vault(), &good).unwrap();
    f.rekey(ArchiveMode::Plaintext, Some(PASSWORD), None);
    let plaintext_copies = vault_core::maintenance::list_snapshots(&paths)
        .unwrap()
        .into_iter()
        .filter(|c| !c.encrypted)
        .map(|c| c.path)
        .collect();
    f.rekey(ArchiveMode::Encrypted, None, Some(NEW_PASSWORD));
    let live_key = Vault::unlock_data_key(&paths, NEW_PASSWORD).unwrap();
    assert_ne!(live_key.expose_bytes(), original_key.expose_bytes());
    vault_core::maintenance::prune(
        &f.vault(),
        &vault_core::RetentionPruneRequest { copies: plaintext_copies, ..Default::default() },
        || vault_core::maintenance::list_snapshots(&paths),
        &Default::default(),
    )
    .unwrap();
    f.session.close_vault().unwrap();
    fs::write(f.root.join(vault_layout::database_path(Db::AgentHistoryEvidence)), b"bad").unwrap();
    let vault = Vault::open_owned(
        paths.clone(),
        vault_core::load_config(&paths).unwrap(),
        Some(&live_key),
        None,
    )
    .unwrap();
    assert!(preview_conversation_repair(&vault, Some(&good), Some(NEW_PASSWORD)).is_err());
    let seal = good.join("snapshot.json");
    let bytes = fs::read(&seal).unwrap();
    let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for proof in bad.as_array_mut().unwrap() {
        if proof["path"] == vault_layout::database_path(Db::AgentHistoryReadModel) {
            proof["sha256"] = "0".repeat(64).into();
        }
    }
    fs::write(&seal, serde_json::to_vec(&bad).unwrap()).unwrap();
    let error = preview_conversation_repair(&vault, Some(&good), Some(PASSWORD)).unwrap_err();
    assert!(error.to_string().contains("snapshot-digest-mismatch"));
    fs::write(seal, bytes).unwrap();
    let plan = preview_conversation_repair(&vault, Some(&good), Some(PASSWORD)).unwrap();
    let installed =
        repair_conversations(&vault, Some(&good), Some(PASSWORD), &plan.token, Default::default())
            .unwrap();
    assert_eq!(installed.data_key.as_ref().unwrap().expose_bytes(), live_key.expose_bytes());
    assert!(Vault::unlock_data_key(&paths, PASSWORD).is_err());
    corpus.check(&installed.vault, Some(NEW_PASSWORD));
}

#[test]
fn legacy_sealed_copy_rebuilds_from_verified_evidence() {
    let f = Fixture::new(false);
    let corpus = Corpus::seed(&f);
    let good = f
        .vault()
        .paths()
        .artifact_path(vault_layout::ArtifactId::NativeSnapshots)
        .join("manual-legacy");
    vault_core::maintenance::snapshot(&f.vault(), &good).unwrap();
    let seal = good.join("snapshot.json");
    let mut proofs: serde_json::Value = serde_json::from_slice(&fs::read(&seal).unwrap()).unwrap();
    pre_pair_copy::replace_roles(&good);
    for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        let role = vault_layout::database_path(db);
        let path = good.join(role);
        let digest = vault_core::utils::sha256_hex(&fs::read(path).unwrap());
        proofs.as_array_mut().unwrap().iter_mut().find(|p| p["path"] == role).unwrap()["sha256"] =
            digest.into();
    }
    fs::write(seal, serde_json::to_vec(&proofs).unwrap()).unwrap();
    f.session.close_vault().unwrap();
    fs::write(f.root.join(vault_layout::database_path(Db::AgentHistoryEvidence)), b"bad").unwrap();
    let vault = f.reopen(false);
    let plan = preview_conversation_repair(&vault, Some(&good), None).unwrap();
    let installed =
        repair_conversations(&vault, Some(&good), None, &plan.token, Default::default()).unwrap();
    corpus.check_rebuilt(&installed.vault, None);
}

#[test]
fn browsing_restore_never_recreates_two_missing_conversation_stores() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let _corpus = Corpus::seed(&f);
        let copies = f.vault().paths().artifact_path(vault_layout::ArtifactId::NativeSnapshots);
        let saved = copies.join("manual-intact");
        vault_core::maintenance::snapshot(&f.vault(), &saved).unwrap();
        f.session.close_vault().unwrap();
        for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            fs::remove_file(f.root.join(vault_layout::database_path(db))).unwrap();
        }
        f.reopen(encrypted);
        let report = vault_worker::run_full_archive_restore(
            &f.session,
            encrypted.then_some(PASSWORD),
            &vault_core::SnapshotRestoreRequest { snapshot_path: saved.display().to_string() },
        )
        .unwrap();
        for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            let path = vault_layout::database_path(db);
            assert!(!f.root.join(path).exists());
            assert!(!std::path::Path::new(&report.quarantine_dir).join(path).exists());
        }
        assert!(
            std::path::Path::new(&report.quarantine_dir)
                .join(vault_layout::UNREADABLE_CONVERSATIONS)
                .is_file()
        );
        assert!(f.vault().conversation_availability().is_err());
    }
}
