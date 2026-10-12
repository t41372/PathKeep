//! Responsibilities: preview and resume verified SQLCipher-to-Turso conversion.
//! Not responsible for: app lifecycle, UI, encryption primitives or AI algorithms.
//! Failure modes: preview omits committed WAL bytes, duplicate resumed rows, stored derived keys,
//! premature swap, lost legacy originals, ignored cancel. Tests: conversion.rs.
pub mod bundle;
mod collisions;
mod enrichments;
mod legacy_recovery;
mod snapshot;
mod source;
#[cfg(any(test, feature = "test-fixtures"))]
pub mod test_support;
mod transfer;
use anyhow::{Context, Result};
pub use legacy_recovery::{list_legacy_snapshots, recover_on_launch};
use serde::{Deserialize, Serialize};
pub use snapshot::restore_snapshot;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
use vault_core::{
    AppConfig, ArchiveMode, ProjectPaths, Vault,
    maintenance::{InstalledArchive, Progress},
};
use vault_store::{CancelToken, OwnerLock};

pub use vault_core::conversion::{Assessment, LockedFile, TableCount};

/// Read-only assessment never opens or initializes a destination Store.
pub fn assess(
    paths: &ProjectPaths,
    _config: &AppConfig,
    password: Option<&str>,
) -> Result<Assessment> {
    let mut tables = Vec::new();
    let required = !paths.app_root.join("archive/history-vault.db").exists()
        && paths.app_root.join("archive/history-vault.sqlite").exists();
    let source_encrypted = paths.app_root.join("archive/history-vault.sqlite").is_file()
        && source::encrypted(&paths.app_root.join("archive/history-vault.sqlite"))?;
    // Without the password an encrypted source can't be counted; the screen still
    // needs to know a conversion is due, how big it is, and to ask for the password.
    let countable = required && (password.is_some() || !source_encrypted);
    let mut bytes = 0;
    let mut retained = Vec::new();
    let mut locked = Vec::new();
    for (db, path) in source::files(paths) {
        if !path.is_file() {
            continue;
        }
        for suffix in ["", "-wal", "-shm"] {
            let mut name = path.as_os_str().to_owned();
            name.push(suffix);
            let file = PathBuf::from(name);
            if file.is_file() {
                bytes += fs::metadata(&file)?.len();
                retained.push(file);
            }
        }
        if !countable {
            continue;
        }
        let Some(conn) = source::open_unless_locked(db, &path, password)? else {
            locked.push(LockedFile { database: format!("{db:?}"), path });
            continue;
        };
        for table in source::tables(&conn)? {
            tables.push(TableCount {
                database: format!("{db:?}"),
                rows: source::count(&conn, &table)?,
                table,
            });
        }
    }
    Ok(Assessment {
        required,
        source_encrypted,
        source_format: "sqlcipher".into(),
        target_format: "turso".into(),
        source_bytes: bytes,
        required_free_bytes: bytes.saturating_mul(3),
        files_touched: vault_core::maintenance::database_files(true)
            .into_iter()
            .map(|p| paths.app_root.join(p))
            .collect(),
        files_retained: retained,
        files_locked: locked,
        tables,
        resumable: stage_root(paths).exists(),
    })
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    cursors: BTreeMap<String, i64>,
    processed: BTreeMap<String, u64>,
    projected: i64,
    /// The source-evidence copy was verified and de-duplication began: its tables no longer hold
    /// what the old file does, so a resumed conversion does not verify them again.
    #[serde(default)]
    evidence_verified: bool,
}
fn stage_root(paths: &ProjectPaths) -> PathBuf {
    paths.staging_dir.join("sqlcipher-conversion")
}
fn save_state(stage: &std::path::Path, state: &State) -> Result<()> {
    vault_core::durable_io::atomic_durable_write(
        &stage.join("conversion.json"),
        &serde_json::to_vec(state)?,
    )
}

/// The caller stops legacy writers before conversion. The owner lock excludes a
/// second app; each native batch is idempotent if a crash precedes its cursor save.
pub fn convert(
    paths: &ProjectPaths,
    config: &AppConfig,
    password: Option<&str>,
    cancel: CancelToken,
    progress: impl FnMut(Progress),
) -> Result<InstalledArchive> {
    convert_owned(paths, config, password, cancel, progress, None)
}

/// Reuses the app's startup reservation when conversion precedes normal unlock.
pub fn convert_owned(
    paths: &ProjectPaths,
    config: &AppConfig,
    password: Option<&str>,
    cancel: CancelToken,
    mut progress: impl FnMut(Progress),
    owner: Option<Arc<OwnerLock>>,
) -> Result<InstalledArchive> {
    let owner = match owner {
        Some(owner) => owner,
        None => Arc::new(OwnerLock::acquire(&paths.app_root)?),
    };
    vault_core::maintenance::recover_native_maintenance(paths)?;
    let assessment = assess(paths, config, password)?;
    anyhow::ensure!(assessment.required, "archive conversion is not required");
    let mut source_config = config.clone();
    source_config.archive_mode =
        if assessment.source_encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext };
    let config = &source_config;
    cancel.check()?;
    let stage = stage_root(paths);
    fs::create_dir_all(&stage)?;
    let target_paths = vault_core::project_paths_with_root(&stage);
    vault_core::config::ensure_paths(&target_paths)?;
    vault_core::save_config(&target_paths, config)?;
    let (vault, key) = if config.archive_mode == ArchiveMode::Encrypted {
        let password = password.context("archive-password-required: enter the archive password")?;
        if stage.join("archive/keys.envelope").exists() {
            let key = Vault::unlock_data_key(&target_paths, password)?;
            (Vault::open_staged(target_paths, config.clone(), Some(&key))?, Some(key))
        } else {
            let (envelope, key) = vault_crypto::Envelope::create(
                password.as_bytes(),
                vault_crypto::KdfParams::RECOMMENDED,
            )?;
            vault_crypto::write_envelope_file(&stage.join("archive/keys.envelope"), &envelope)?;
            (Vault::open_staged(target_paths, config.clone(), Some(&key))?, Some(key))
        }
    } else {
        (Vault::open_staged(target_paths, config.clone(), None)?, None)
    };
    let mut state = match fs::read(stage.join("conversion.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::default(),
        Err(error) => return Err(error.into()),
    };
    let mut left_behind = Vec::new();
    let mut evidence_copied = false;
    for (db, path) in source::files(paths) {
        if !path.is_file() {
            continue;
        }
        let Some(source) = source::open_unless_locked(db, &path, password)? else {
            left_behind.push(db);
            continue;
        };
        let target_tables = vault.store().read(db, |s| {
            s.query("SELECT name FROM sqlite_schema WHERE type='table'", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })?;
        let mut tables = source::tables(&source)?;
        tables.retain(|table| target_tables.contains(table));
        let sequence_tables = tables.clone();
        if db == vault_store::Db::Intelligence {
            collisions::convert(&vault, &source, &cancel, &mut progress)?;
            tables.retain(|table| !collisions::handles(table));
            // Before embeddings: their content hashes include each page's enrichment.
            if enrichments::present(&source)? {
                let name = "Intelligence.visit_content_enrichments".to_owned();
                let after = state.cursors.get(&name).copied().unwrap_or(0);
                enrichments::convert(
                    &vault,
                    &source,
                    after,
                    &cancel,
                    |cursor, processed| {
                        state.cursors.insert(name.clone(), cursor);
                        state.processed.insert(name.clone(), processed);
                        save_state(&stage, &state)
                    },
                    &mut progress,
                )?;
            }
        }
        tables.sort_by_key(|t| match t.as_str() {
            "favicon_blobs" | "og_image_blobs" => 0,
            "runs" => 1,
            "source_profiles" | "conversations" => 2,
            "urls" => 3,
            "ai_embeddings" => 6,
            _ => 5,
        });
        for table in &tables {
            let fields = transfer::fields(&vault, db, &source, table)?;
            let name = format!("{db:?}.{table}");
            let total = source::count(&source, table)?;
            let mut cursor = state.cursors.get(&name).copied();
            let mut processed = state.processed.get(&name).copied().unwrap_or(0);
            loop {
                cancel.check()?;
                let rows = source::page(&source, table, &fields, cursor)?;
                if rows.is_empty() {
                    break;
                }
                cursor = rows.last().map(|r| r.id);
                processed += rows.len() as u64;
                transfer::write(&vault, db, table, &fields, rows)?;
                state.cursors.insert(name.clone(), cursor.unwrap());
                state.processed.insert(name.clone(), processed);
                save_state(&stage, &state)?;
                progress(Progress {
                    phase: "converting".into(),
                    database: Some(format!("{db:?}")),
                    table: Some(table.clone()),
                    processed,
                    total,
                });
            }
        }
        transfer::sequences(&vault, db, &source, &sequence_tables)?;
        if db == vault_store::Db::SourceEvidence {
            evidence_copied = true;
            if state.evidence_verified {
                continue;
            }
        }
        for table in &tables {
            cancel.check()?;
            let fields = transfer::fields(&vault, db, &source, table)?;
            let total = source::count(&source, table)?.saturating_mul(2);
            let mut verifying = |processed| {
                progress(Progress {
                    phase: "verifying".into(),
                    database: Some(format!("{db:?}")),
                    table: Some(table.clone()),
                    processed,
                    total,
                })
            };
            verifying(0);
            transfer::verify(&vault, db, &source, table, &fields, &cancel, &mut verifying)?;
            progress(Progress {
                phase: "verifying".into(),
                database: Some(format!("{db:?}")),
                table: Some(table.clone()),
                processed: total,
                total,
            });
        }
    }
    if evidence_copied {
        // The old format stored the same evidence again for every import that read it.
        state.evidence_verified = true;
        save_state(&stage, &state)?;
        vault_core::conversion::dedupe_source_evidence(&vault, &cancel)?;
    }
    if left_behind.contains(&vault_store::Db::SourceEvidence) {
        // The watermarks name the newest evidence batch of each profile; those batches
        // stayed in the file that wasn't carried over.
        vault.store().write(vault_store::Db::Archive, vault_store::Priority::Batch, |s| {
            s.execute("UPDATE profile_watermarks SET last_source_batch_id=NULL", ())?;
            Ok(())
        })?;
    }
    // The same visit from a Takeout import and a browser counts once from now on; the old
    // format kept and counted both. Idempotent, so a resumed conversion runs it again.
    if state.projected == 0 {
        cancel.check()?;
        vault_core::conversion::link_same_visits(&vault)?;
        // Every page was given its site by this build's public suffix list (`transfer`).
        vault_core::archive::sites::record_list_version(&vault)?;
    }
    while let Some(last) =
        vault_core::conversion::project_pages(&vault, state.projected, source::PAGE)?
    {
        cancel.check()?;
        state.projected = last;
        save_state(&stage, &state)?;
        progress(Progress {
            phase: "projecting".into(),
            database: Some("Archive".into()),
            table: Some("pages".into()),
            processed: last as u64,
            total: 0,
        });
    }
    vault_core::conversion::refresh_totals(&vault)?;
    vault_core::conversion::refresh_embedding_totals(&vault)?;
    vault_core::conversion::project_enrichments(&vault, &cancel)?;
    // Insights are derived data and aren't converted; the first run rebuilds them.
    vault_core::intelligence_runtime::enqueue_deterministic_rebuild_job(
        &vault,
        &vault_core::CoreIntelligenceRebuildRequest { full_rebuild: true, ..Default::default() },
        "archive converted from the old format",
    )?;
    cancel.check()?;
    progress(Progress {
        phase: "indexing".into(),
        database: None,
        table: None,
        processed: 0,
        total: 0,
    });
    rebuild_index(&vault, &cancel, &mut progress)?;
    progress(Progress {
        phase: "publishing".into(),
        database: None,
        table: None,
        processed: 0,
        total: 0,
    });
    vault.close()?;
    cancel.check()?;
    let previous = vault_core::maintenance::install::install(
        paths,
        &stage,
        vault_core::maintenance::database_files(true),
        true,
    )?;
    let opened = Vault::open_owned(paths.clone(), config.clone(), key.as_ref(), Some(owner))?;
    Ok(InstalledArchive { vault: opened, data_key: key, previous_files: previous })
}

fn rebuild_index(
    vault: &Vault,
    cancel: &CancelToken,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    let (send, receive) = std::sync::mpsc::channel();
    let indexer = vault.search().clone();
    let token = cancel.clone();
    std::thread::scope(|scope| {
        let build = scope.spawn(move || {
            indexer.rebuild_with_control(token, move |p| {
                let _ = send.send(p);
            })
        });
        for p in receive {
            progress(Progress {
                phase: "indexing".into(),
                database: None,
                table: None,
                processed: p.documents,
                total: 0,
            });
        }
        build.join().map_err(|_| anyhow::anyhow!("index rebuild worker panicked"))?
    })
}
