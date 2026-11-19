//! Responsibilities: exercise the literal root-operation list against future control bytes.
//! Not responsible for: deriving the list from production registration or replacing real owners.
use super::support::{self, FUTURE, Fixture, PASSWORD, WRITER, error};
use std::{fs, path::PathBuf, process::Command, sync::Arc};
use vault_core::{ArchiveMode, Vault};

#[derive(Clone, Copy, Debug)]
enum Operation {
    LoadConfig,
    SaveConfig,
    Open,
    OpenStaged,
    OpenPassword,
    Initialize,
    SessionOpen,
    SessionStartup,
    SessionUnlock,
    WipeRecovery,
    InstallRecovery,
    LegacyRecovery,
    Conversion,
    Assessment,
    SaveUserConfig,
    Wipe,
    PreviewWipe,
    Snapshot,
    Restore,
    FullRestore,
    Export,
    Import,
    PreviewImport,
    ModeChange,
    Rewrap,
    Reconcile,
    WorkerRekey,
    ScheduleApply,
    ScheduleRemove,
    ScheduleRepair,
    AppLock,
    SnapshotList,
    RetentionPreview,
    RetentionPrune,
    NativeLaunchRecovery,
    KeyringClear,
    ProviderSet,
    ProviderClear,
    AppSnapshot,
    Diagnostics,
    FrontendError,
    KeyringRead,
    KeyringStore,
    KeyringStatus,
    CheckpointPreview,
    CheckpointReplay,
    CoreBackup,
    ModelInstall,
    SurfacePolicy,
}

#[derive(Debug)]
struct Observation {
    result: String,
    before: support::tree::Census,
    after: support::tree::Census,
    outside_before: support::tree::Census,
    outside_after: support::tree::Census,
    secret_reads_before: usize,
    secret_reads_after: usize,
}

fn observe(operation: Operation, control: &str) -> Observation {
    use Operation::*;
    let live = matches!(
        operation,
        Snapshot
            | Restore
            | FullRestore
            | Export
            | Import
            | PreviewImport
            | ModeChange
            | Rewrap
            | Reconcile
            | RetentionPrune
            | NativeLaunchRecovery
            | CheckpointPreview
            | CheckpointReplay
            | CoreBackup
    );
    let mut f = if matches!(operation, Rewrap | KeyringStore) {
        Fixture::encrypted()
    } else {
        Fixture::new(live)
    };
    let source = if matches!(operation, CheckpointPreview | CheckpointReplay) {
        Some(f.browser_checkpoint())
    } else if matches!(operation, Restore) {
        Some(f.snapshot_source())
    } else if matches!(operation, FullRestore) {
        let source = f.root.join("raw-snapshots/maintenance/admission-source");
        vault_core::maintenance::snapshot(f.vault(), &source).unwrap();
        Some(source)
    } else if matches!(operation, Import | PreviewImport) {
        Some(f.bundle_source())
    } else {
        None
    };
    if matches!(operation, Restore | FullRestore | Import) {
        f.session.close_vault().unwrap();
        f.vault.take().unwrap().close().unwrap();
    }
    if matches!(operation, WipeRecovery | Initialize) {
        f.write(".pk-wipe-in-progress.json", r#"{"providerIds":[],"schedule":null}"#);
        f.write("derived/agent.db", b"wipe must not erase this sentinel");
    }
    if matches!(operation, InstallRecovery | SessionStartup) {
        f.seed_install();
    }
    if matches!(operation, LegacyRecovery) {
        let mut header = vec![0u8; 128];
        header[..16].copy_from_slice(b"SQLite format 3\0");
        f.write("archive/history-vault.sqlite", header);
    }
    if matches!(operation, KeyringClear | KeyringRead | KeyringStatus) {
        vault_platform::keyring_set_database_key("synthetic remembered key").unwrap();
    }
    if matches!(operation, ProviderClear) {
        vault_platform::keyring_set_provider_api_key("fixture", "synthetic provider key").unwrap();
    }
    let plan = if matches!(operation, ScheduleApply | ScheduleRemove | ScheduleRepair) {
        Some(
            vault_platform::preview_schedule(
                Some("macos"),
                &PathBuf::from("fixture-worker"),
                &f.paths(),
                &vault_platform::ScheduleParameters {
                    due_after_hours: 24.,
                    check_interval_hours: 6.,
                },
            )
            .unwrap(),
        )
    } else {
        None
    };
    f.write(
        "config.json",
        if matches!(operation, Rewrap | KeyringStore | OpenPassword) {
            control.replace("Plaintext", "Encrypted")
        } else {
            control.into()
        },
    );
    let before = f.observe();
    let outside_before = f.outside();
    let secret_reads_before =
        vault_platform::test_support::keyring_secret_read_count(&f.temp.path().join("keyring"));
    let result = match operation {
        LoadConfig => error(vault_core::load_config(&f.paths())),
        SaveConfig => {
            error(vault_core::save_config(&vault_core::ControlGate::acquire(&f.paths()), &f.config))
        }
        Open => error(Vault::open(f.paths(), f.config.clone(), None).map(|v| v.close())),
        OpenStaged => {
            error(Vault::open_staged(f.paths(), f.config.clone(), None).map(|v| v.close()))
        }
        OpenPassword => {
            let mut config = f.config.clone();
            config.archive_mode = ArchiveMode::Encrypted;
            error(Vault::open_password(f.paths(), config, PASSWORD, None).map(|(v, _)| v.close()))
        }
        Initialize => error(vault_worker::initialize_archive_database_with_progress(
            &f.session,
            &f.config,
            None,
            |_| {},
        )),
        SessionOpen => error(f.session.vault()),
        SessionStartup => error(f.session.finish_startup()),
        SessionUnlock => error(f.session.unlock_from_keychain()),
        WipeRecovery => error(vault_worker::finish_interrupted_data_wipe()),
        InstallRecovery => error(vault_core::maintenance::install::recover_before_open(&f.paths())),
        LegacyRecovery => {
            let mut stale = f.config.clone();
            stale.archive_mode = ArchiveMode::Encrypted;
            error(vault_convert_sqlcipher::recover_on_launch(&f.paths(), &stale))
        }
        Conversion => error(vault_convert_sqlcipher::convert(
            &f.paths(),
            &f.config,
            None,
            Default::default(),
            |_| {},
        )),
        Assessment => error(vault_worker::assess_archive_upgrade(None)),
        SaveUserConfig => error(vault_worker::save_user_config(&f.config, None)),
        Wipe => error(vault_worker::wipe_all_data("DELETE", || Ok(()))),
        PreviewWipe => error(vault_worker::preview_data_wipe()),
        Snapshot => {
            error(vault_core::maintenance::snapshot(f.vault(), &f.temp.path().join("output")))
        }
        Restore => error(vault_core::maintenance::restore_snapshot_to(
            &f.paths(),
            None,
            None,
            source.as_deref().unwrap(),
            None,
            None,
            Default::default(),
        )),
        FullRestore => error(vault_worker::run_full_archive_restore(
            &f.session,
            None,
            &vault_core::SnapshotRestoreRequest {
                snapshot_path: source.unwrap().display().to_string(),
            },
        )),
        Export => error(vault_worker::export_app_data(
            f.vault(),
            f.temp.path().join("output.pkexport"),
            PASSWORD,
        )),
        Import => error(vault_core::migration::native::import(
            &f.paths(),
            None,
            None,
            source.as_deref().unwrap(),
            &vault_core::ApplyImportOptions {
                source_archive_key: Some(PASSWORD.into()),
                confirm_overwrite: true,
            },
            Default::default(),
        )),
        PreviewImport => error(vault_worker::preview_import(&f.paths(), source.unwrap())),
        ModeChange => error(vault_core::maintenance::switch_mode(
            f.vault(),
            None,
            ArchiveMode::Encrypted,
            Some(PASSWORD),
            Default::default(),
            |_| {},
        )),
        Rewrap => error(vault_core::maintenance::rewrap_password(
            f.vault(),
            PASSWORD,
            "another fixture password",
        )),
        Reconcile => error(vault_core::maintenance::reconcile(f.vault())),
        WorkerRekey => error(vault_worker::rekey_archive_database(
            &f.session,
            &vault_worker::RekeyRequest {
                new_mode: ArchiveMode::Encrypted,
                current_key: None,
                new_key: Some(PASSWORD.into()),
                remember_in_keychain: None,
            },
        )),
        ScheduleApply => error(vault_worker::apply_schedule_plan(plan.as_ref().unwrap())),
        ScheduleRemove => error(vault_worker::remove_schedule_plan(plan.as_ref().unwrap())),
        ScheduleRepair => error(vault_worker::repair_schedule_plan(plan.as_ref().unwrap())),
        AppLock => error(vault_worker::lock_app_ui_session(None)),
        SnapshotList => error(vault_worker::list_recovery_snapshots()),
        RetentionPreview => error(vault_worker::preview_retention_plan()),
        RetentionPrune => error(vault_worker::run_retention_plan(
            f.vault(),
            &vault_core::RetentionPruneRequest {
                bucket_ids: vec!["staging".into()],
                ..Default::default()
            },
        )),
        NativeLaunchRecovery => {
            let mut stale = f.config.clone();
            stale.archive_mode = ArchiveMode::Encrypted;
            error(vault_core::recover_archive_on_launch(&f.paths(), &stale))
        }
        KeyringClear => error(vault_worker::clear_database_key_from_keyring()),
        ProviderSet => error(vault_worker::store_ai_provider_api_key(
            &vault_core::AiProviderSecretInput {
                provider_id: "fixture".into(),
                api_key: "synthetic new provider key".into(),
            },
            None,
            None,
        )),
        ProviderClear => error(vault_worker::clear_ai_provider_api_key("fixture", None, None)),
        AppSnapshot => error(vault_worker::app_snapshot(None)),
        Diagnostics => error(vault_core::load_runtime_diagnostics(&f.paths())),
        FrontendError => error(vault_core::record_frontend_error(
            &f.paths(),
            &vault_core::FrontendErrorReportRequest {
                message: "synthetic renderer failure".into(),
                ..Default::default()
            },
        )),
        KeyringRead => error(vault_worker::read_database_key_from_keyring()),
        KeyringStore => error(vault_worker::write_database_key_to_keyring(PASSWORD)),
        KeyringStatus => vault_worker::keyring_report().message.unwrap_or_else(|| "SUCCESS".into()),
        CheckpointPreview => error(vault_worker::preview_snapshot_restore_plan(
            f.vault(),
            &vault_core::SnapshotRestoreRequest {
                snapshot_path: source.unwrap().display().to_string(),
            },
        )),
        CheckpointReplay => error(vault_worker::run_snapshot_restore_plan(
            f.vault(),
            &vault_core::SnapshotRestoreRequest {
                snapshot_path: source.unwrap().display().to_string(),
            },
        )),
        ModelInstall => error(vault_core::ensure_model_downloaded(
            &f.paths(),
            vault_core::DEFAULT_STATIC_MODEL_REPO,
            vault_core::DEFAULT_STATIC_MODEL_FILES,
            false,
            &mut vault_core::NoopDownloadProgress,
        )),
        SurfacePolicy => {
            use kinds::backups;
            use serde_json::{from_value, json};
            use vault_worker::data_kinds::{self as kinds, preview};
            let quiet: backups::Events = Arc::new(|_, _| {});
            let failures = [
                error(backups::enable(&f.session,from_value(json!({"previewId":"preview","selectedRootIds":[],"expectedPolicyToken":"old","readGit":false,"requestId":"id"})).unwrap(),quiet.clone())),
                error(backups::start(&f.session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":"id"})).unwrap(),quiet.clone())),
                error(backups::list_runs(&f.session,from_value(json!({"includeArchiveWide":true,"limit":1})).unwrap())),
                error(backups::get_run(&f.session,from_value(json!({"runRef":"agent-history:test"})).unwrap())),
                error(backups::overview(&f.session,from_value(json!({"purpose":"backup"})).unwrap())),
                error(backups::sources(&f.session,from_value(json!({"limit":1})).unwrap())),
                error(backups::tasks(&f.session,from_value(json!({"group":"attention","limit":1})).unwrap())),
                error(backups::task_summary(&f.session,from_value(json!({})).unwrap())),
                error(backups::control(&f.session,from_value(json!({"taskId":"task","action":"retry","expectedToken":"old","requestId":"id"})).unwrap(),quiet)),
                error(kinds::list(&f.session)),
                error(kinds::settings()),
                error(kinds::retry_agent_store(&f.session,|_|{})),
                error(kinds::dismiss_hint(from_value(json!({"hint":"intro"})).unwrap(),Arc::new(|_,_|{}))),
                error(kinds::set_state(
                    &f.session,
                    from_value(json!({"kind":"browsing","state":"on","expectedPolicyToken":"old"}))
                        .unwrap(),
                    |_| {},
                )),
                error(kinds::update_sources(
                    &f.session,
                    from_value(json!({"readGit":false,"expectedToken":"old"})).unwrap(),
                    |_| {},
                )),
                error(preview::detect(from_value(json!({"kinds":["agent-history"]})).unwrap())),
                error(
                    preview::start(&f.session, from_value(json!({"purpose":"rescan"})).unwrap())
                        .map(drop),
                ),
                error(preview::cancel(
                    from_value(json!({"requestIdOrStreamId":"unknown"})).unwrap(),
                )),
            ];
            assert!(failures.iter().all(|result| result == &failures[0]), "{failures:?}");
            failures[0].clone()
        }
        CoreBackup => error(vault_core::run_backup(f.vault(), None)),
    };
    let after = f.observe();
    let outside_after = f.outside();
    let secret_reads_after =
        vault_platform::test_support::keyring_secret_read_count(&f.temp.path().join("keyring"));
    Observation {
        result,
        before,
        after,
        outside_before,
        outside_after,
        secret_reads_before,
        secret_reads_after,
    }
}

// Each row is deliberate. A future registration cannot generate its own acceptance row.
macro_rules! refuses {
    ($name:ident, $operation:ident) => {
        #[test]
        fn $name() {
            // The build is level 1: a reader floor of 2 is too new to read, a writer floor of 2
            // too new to write. Each has its own reason, and neither may change anything.
            let observations =
                [(FUTURE, "archive-reader-too-old"), (WRITER, "archive-writer-too-old")]
                    .map(|(control, code)| (code, observe(Operation::$operation, control)));
            let failures: Vec<_> = observations
                .iter()
                .filter(|(code, observed)| {
                    !observed.result.contains(code)
                        || observed.before != observed.after
                        || observed.outside_before != observed.outside_after
                        || observed.secret_reads_before != observed.secret_reads_after
                })
                .collect();
            assert!(failures.is_empty(), "{failures:#?}");
        }
    };
}
refuses!(load_config_refuses_before_defaulting, LoadConfig);
refuses!(config_save_refuses_before_scaffolding_or_replacement, SaveConfig);
refuses!(vault_open_checks_disk_control_even_with_a_caller_config, Open);
refuses!(staged_open_checks_its_own_control, OpenStaged);
refuses!(password_open_refuses_before_envelope_creation, OpenPassword);
refuses!(initialization_refuses_before_wipe_recovery, Initialize);
refuses!(owner_session_open_refuses, SessionOpen);
refuses!(owner_startup_refuses_before_install_recovery, SessionStartup);
refuses!(owner_unlock_refuses_before_keychain_access, SessionUnlock);
refuses!(interrupted_wipe_refuses_before_deletion, WipeRecovery);
refuses!(install_recovery_refuses_before_cleanup, InstallRecovery);
refuses!(legacy_recovery_refuses_before_repair, LegacyRecovery);
refuses!(conversion_refuses_before_owner_reservation, Conversion);
refuses!(upgrade_assessment_does_not_default_future_control, Assessment);
refuses!(settings_save_refuses, SaveUserConfig);
refuses!(wipe_refuses, Wipe);
refuses!(wipe_preview_refuses, PreviewWipe);
refuses!(snapshot_reinspects_the_live_owner_root, Snapshot);
refuses!(restore_inspects_future_target_without_a_current_vault, Restore);
refuses!(full_restore_never_treats_future_control_as_damage_to_replace, FullRestore);
refuses!(move_export_reinspects_the_live_owner_root, Export);
refuses!(move_import_inspects_future_target_without_a_current_vault, Import);
refuses!(move_preview_inspects_target_control, PreviewImport);
refuses!(mode_change_reinspects_the_live_owner_root, ModeChange);
refuses!(password_change_reinspects_the_live_owner_root, Rewrap);
refuses!(mode_reconciliation_reinspects_control, Reconcile);
refuses!(worker_rekey_refuses, WorkerRekey);
refuses!(schedule_install_refuses_before_machine_effects, ScheduleApply);
refuses!(schedule_removal_refuses_before_machine_effects, ScheduleRemove);
refuses!(schedule_repair_refuses_before_machine_effects, ScheduleRepair);
refuses!(app_lock_refuses_before_session_state_changes, AppLock);
refuses!(snapshot_listing_does_not_hide_future_control, SnapshotList);
refuses!(retention_preview_refuses, RetentionPreview);
refuses!(retention_execution_refuses, RetentionPrune);
refuses!(native_launch_recovery_refuses_before_mode_healing, NativeLaunchRecovery);
refuses!(remembered_key_clear_refuses_before_machine_effects, KeyringClear);
refuses!(provider_key_store_refuses_before_machine_effects, ProviderSet);
refuses!(provider_key_clear_refuses_before_machine_effects, ProviderClear);
refuses!(app_snapshot_refuses, AppSnapshot);
refuses!(diagnostics_read_refuses_before_scaffolding, Diagnostics);
refuses!(frontend_error_recording_refuses_before_root_logging, FrontendError);
refuses!(remembered_key_read_refuses_before_secret_access, KeyringRead);
refuses!(remembered_key_store_refuses_before_machine_effects, KeyringStore);
refuses!(keyring_status_refuses_before_reading_machine_state, KeyringStatus);
refuses!(checkpoint_preview_refuses_before_reading_source_data, CheckpointPreview);
refuses!(checkpoint_replay_refuses_before_writing_runs, CheckpointReplay);
refuses!(app_surface_refuses_before_touching_sources_or_policy, SurfacePolicy);
refuses!(core_backup_reinspects_the_owner_root, CoreBackup);

#[test]
fn every_worker_cli_route_refuses_through_the_real_owner_without_root_changes() {
    // This is an already-running owner's boundary, not a mock response server.
    let mut failures = Vec::new();
    for args in [
        vec!["ping"],
        vec!["doctor"],
        vec!["backup"],
        vec!["backup", "--due-only"],
        vec!["ai-index"],
        vec!["ai-queue"],
        vec!["ai-jobs"],
        vec!["intelligence-jobs"],
        vec!["mcp-server"],
    ] {
        let f = Fixture::new(false);
        let owner = Arc::new(vault_store::OwnerLock::acquire(&f.root).unwrap());
        f.session.reserve_owner(owner);
        let session = f.session.clone();
        let server = vault_owner_ipc::Server::start(&f.root, move |request, progress| {
            vault_worker::owner_requests::dispatch(request, progress, session.get_key(), || {
                session.background_vault().map_err(anyhow::Error::msg)
            })
        })
        .unwrap();
        f.write("config.json", FUTURE);
        if args == ["backup", "--due-only"] {
            f.write("schedule/logs/worker.stdout.log", vec![b'x'; 4_194_305]);
        }
        let before = f.observe();
        let outside = f.outside();
        let output =
            Command::new(env!("CARGO_BIN_EXE_pathkeep-worker")).args(&args).output().unwrap();
        let detail = String::from_utf8_lossy(&output.stderr);
        let after = f.observe();
        if output.status.code() != Some(1)
            || !detail.contains("archive-reader-too-old")
            || before != after
            || outside != f.outside()
        {
            failures.push(format!(
                "{args:?}: status={:?}, error={detail}, before={before:?}, after={after:?}",
                output.status.code()
            ));
        }
        drop(server);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn restore_refuses_a_future_source_without_touching_source_or_target() {
    let f = Fixture::new(true);
    let source = f.snapshot_source();
    fs::write(source.join("config.json"), FUTURE).unwrap();
    // Reseal only the hand-written fixture control; the file-set expectation stays literal.
    let proof_path = source.join("snapshot.json");
    let mut proof: serde_json::Value =
        serde_json::from_slice(&fs::read(&proof_path).unwrap()).unwrap();
    for row in proof.as_array_mut().unwrap() {
        if row["path"] == "config.json" {
            row["sha256"] = vault_core::utils::sha256_hex(FUTURE.as_bytes()).into();
        }
    }
    fs::write(proof_path, serde_json::to_vec(&proof).unwrap()).unwrap();
    let before = f.observe();
    let source_before = support::tree::census(&source, vault_core::utils::sha256_hex);
    let result = error(vault_core::maintenance::restore_snapshot(
        f.vault(),
        &source,
        None,
        Default::default(),
    ));
    let after = f.observe();
    let source_after = support::tree::census(&source, vault_core::utils::sha256_hex);
    assert!(
        result.contains("archive-reader-too-old")
            && before == after
            && source_before == source_after,
        "result={result}; before={before:?}; after={after:?}; source-before={source_before:?}; source-after={source_after:?}"
    );
}

#[test]
fn every_mcp_tool_rechecks_the_owner_root_after_connection() {
    let mut failures = Vec::new();
    for (tool, arguments) in [
        ("initialize", serde_json::json!({})),
        ("search-history", serde_json::json!({"query":"fixture"})),
        ("archive-status", serde_json::json!({})),
        ("usage-guide", serde_json::json!({})),
    ] {
        let f = Fixture::new(false);
        f.session.reserve_owner(Arc::new(vault_store::OwnerLock::acquire(&f.root).unwrap()));
        let session = f.session.clone();
        let server = vault_owner_ipc::Server::start(&f.root, move |request, progress| {
            vault_worker::owner_requests::dispatch(request, progress, session.get_key(), || {
                session.background_vault().map_err(anyhow::Error::msg)
            })
        })
        .unwrap();
        f.write("config.json", FUTURE);
        let before = f.observe();
        let outside = f.outside();
        let result = error(tokio::runtime::Runtime::new().unwrap().block_on(
            vault_owner_ipc::Client::new(&f.root).request(
                vault_owner_ipc::Request::new(
                    "mcp",
                    serde_json::json!({"tool":tool,"arguments":arguments}),
                ),
                |_| {},
            ),
        ));
        let after = f.observe();
        if !result.contains("archive-reader-too-old") || before != after || outside != f.outside() {
            failures.push(format!("{tool}: result={result}; before={before:?}; after={after:?}"));
        }
        drop(server);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// The worker command line refuses in its own process, before it connects. This asks the owner
// directly, as anything that speaks its protocol can, for each route that uses the archive.
// `ping` is left out on purpose: it answers with the owner's process id and touches nothing.
#[test]
fn every_owner_route_that_uses_the_archive_refuses_at_the_owner() {
    let mut failures = Vec::new();
    for (method, params) in [
        ("backup", serde_json::json!({"dueOnly": true})),
        ("backup", serde_json::json!({"dueOnly": false})),
        ("doctor", serde_json::json!({})),
        ("ai-index", serde_json::json!({})),
        ("ai-queue", serde_json::json!({})),
        ("ai-jobs", serde_json::json!({})),
        ("intelligence-jobs", serde_json::json!({})),
    ] {
        let f = Fixture::new(false);
        f.session.reserve_owner(Arc::new(vault_store::OwnerLock::acquire(&f.root).unwrap()));
        let session = f.session.clone();
        let server = vault_owner_ipc::Server::start(&f.root, move |request, progress| {
            vault_worker::owner_requests::dispatch(request, progress, session.get_key(), || {
                session.background_vault().map_err(anyhow::Error::msg)
            })
        })
        .unwrap();
        f.write("config.json", FUTURE);
        let before = f.observe();
        let outside = f.outside();
        let result = error(
            tokio::runtime::Runtime::new().unwrap().block_on(
                vault_owner_ipc::Client::new(&f.root)
                    .request(vault_owner_ipc::Request::new(method, params.clone()), |_| {}),
            ),
        );
        let after = f.observe();
        if !result.contains("archive-reader-too-old") || before != after || outside != f.outside() {
            failures.push(format!(
                "{method} {params}: result={result}; before={before:?}; after={after:?}"
            ));
        }
        drop(server);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
refuses!(model_install_refuses_before_touching_generations, ModelInstall);
