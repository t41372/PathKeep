//! Responsibilities: import the whole-app files v0.3 exported (bundle format 1: the old
//! SQLCipher databases, a plain config, and the audit, raw-snapshots and sidecars folders) by
//! converting them the way an archive left on disk is converted, then installing the result
//! the way a native bundle is installed.
//! Not responsible for: native bundles (`vault_core::migration::native`) or the table copy
//! itself (`convert_owned`).
//! Failure modes: a wrong password reaches this machine's archive; an import that stops
//! partway leaves the archive replaced; paths of the exporting computer stay in snapshot and
//! audit rows; the old files are installed next to the native ones; the preview promises chats,
//! which format 1 never carried. Tests: upgrade.spec (a file exported by PathKeep 0.3.0 opens
//! and replaces the archive).
use anyhow::{Context, Result};
use std::{fs, path::Path, sync::Arc};
use vault_core::{
    ApplyImportOptions, ImportExclusionNote, ImportPreview, ImportResult, LanguagePreference,
    ProjectPaths, Vault,
    maintenance::{InstalledArchive, Progress},
    migration::{self, native},
};
use vault_store::{CancelToken, Db};

/// The manifest version v0.3 wrote.
pub const FORMAT: u32 = 1;

/// Same checks as a native preview; the inventory is authenticated before any password.
pub fn preview(paths: &ProjectPaths, bundle: &Path) -> Result<ImportPreview> {
    let manifest = migration::read_and_validate_manifest(bundle)?;
    anyhow::ensure!(
        manifest.format_version == FORMAT,
        "unsupported bundle format {}",
        manifest.format_version
    );
    anyhow::ensure!(
        manifest.files.iter().any(|f| f.path == "archive/history-vault.sqlite"),
        "bundle is missing required entry archive/history-vault.sqlite"
    );
    for file in &manifest.files {
        migration::validate_bundle_relative_path(&file.path)?;
    }
    let mut exclusion_notes = native::exclusion_notes(paths);
    let chats = match vault_core::load_config(paths).unwrap_or_default().preferred_language {
        LanguagePreference::ZhCn => {
            "这个文件由 PathKeep 0.3 导出，不含助理对话；它们仍在原来的电脑上。"
        }
        LanguagePreference::ZhTw => {
            "這個檔案由 PathKeep 0.3 匯出，不含助理對話；它們仍在原來的電腦上。"
        }
        _ => {
            "This file was exported by PathKeep 0.3, which didn't include Ask chats; they are still on the other computer."
        }
    };
    exclusion_notes
        .push(ImportExclusionNote { path: "derived/agent.sqlite".into(), reason: chats.into() });
    Ok(ImportPreview {
        schema_up_to_date: false,
        migrations_to_apply: Vec::new(),
        bytes_to_extract: manifest.files.iter().map(|f| f.size_bytes).sum(),
        will_overwrite_existing: native::will_overwrite(paths),
        manifest,
        exclusion_notes,
    })
}

/// Unpacks the bundle as the old app's data folder inside a staging folder, converts it there,
/// and installs the converted archive. This machine's archive is untouched until the install.
pub fn import(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    reservation: Option<Arc<vault_store::OwnerLock>>,
    bundle: &Path,
    options: &ApplyImportOptions,
    cancel: CancelToken,
    progress: impl FnMut(Progress),
) -> Result<(ImportResult, InstalledArchive)> {
    let _operation = vault_core::archive::ArchiveOpGate::acquire(paths);
    let owner = match current {
        Some(vault) => vault.store().owner()?,
        None => match reservation {
            Some(owner) => owner,
            None => Arc::new(vault_store::OwnerLock::acquire(&paths.app_root)?),
        },
    };
    vault_core::maintenance::recover_native_maintenance(paths)?;
    let preview = preview(paths, bundle)?;
    anyhow::ensure!(
        options.confirm_overwrite || !preview.will_overwrite_existing,
        "confirmOverwrite is required"
    );
    let password = options.source_archive_key.as_deref();
    if preview.manifest.archive_mode == "encrypted" && password.is_none() {
        anyhow::bail!(migration::IMPORT_SOURCE_KEY_REQUIRED_PREFIX);
    }
    let stage = vault_core::maintenance::staging_root(paths)?;
    let old_root = stage.path().join("bundle");
    migration::extract_bundle_into_cancel(bundle, &old_root, &preview.manifest, &cancel)?;
    let old_paths = vault_core::project_paths_with_root(&old_root);
    vault_core::config::ensure_paths(&old_paths)?;
    if old_root.join("config/config.json").is_file() {
        fs::rename(old_root.join("config/config.json"), &old_paths.config_path)?;
    }
    let mut config = vault_core::load_config(&old_paths).unwrap_or_default();
    config.initialized = true;
    native::keep_machine_settings(&mut config, &vault_core::load_config(paths)?);
    vault_core::save_config(&old_paths, &config)?;
    let converted =
        crate::convert_owned(&old_paths, &config, password, cancel.clone(), progress, None)?;
    if let Some(source_root) = exporting_root(&converted.vault)? {
        native::adopt_moved_paths(
            &converted.vault,
            &old_root,
            &source_root,
            &paths.app_root,
            &cancel,
        )?;
    }
    converted.vault.close()?;
    drop(converted.vault);
    remove_old_files(&old_root)?;
    let config = vault_core::load_config(&old_paths)?;
    let installed = native::install_staged(
        paths,
        current,
        owner,
        &old_root,
        config,
        converted.data_key,
        &cancel,
    )?;
    let report = ImportResult {
        manifest: preview.manifest,
        migrations_applied: Vec::new(),
        final_schema_version: vault_core::store_schema::schema_version(vault_store::Db::Archive),
        preserved_previous_as_bak: preview.will_overwrite_existing,
    };
    Ok((report, installed))
}

/// v0.3 stored absolute paths into its data folder (`…/raw-snapshots/…`, `…/audit/…`) and,
/// unlike a native bundle, didn't record that folder. Any one such path gives it back.
fn exporting_root(vault: &Vault) -> Result<Option<String>> {
    for (table, column, folder) in [
        ("snapshots", "file_path", "raw-snapshots"),
        ("import_batches", "audit_path", "audit"),
        ("manifests", "file_path", "audit"),
    ] {
        let paths = vault.store().read(Db::Archive, move |s| {
            s.query(
                &format!("SELECT {column} FROM {table} WHERE {column} IS NOT NULL LIMIT 16"),
                (),
                |r| Ok(r.get::<String>(0)?),
            )
        })?;
        for path in paths {
            let normalized = path.replace('\\', "/");
            if let Some(at) = normalized.find(&format!("/{folder}/")) {
                return Ok(Some(path[..at].to_owned()));
            }
        }
    }
    Ok(None)
}

/// The old databases were converted; only native files and the moved folders are installed.
fn remove_old_files(old_root: &Path) -> Result<()> {
    for folder in ["archive", "derived"] {
        let Ok(entries) = fs::read_dir(old_root.join(folder)) else { continue };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if [".sqlite", ".sqlite-wal", ".sqlite-shm", ".sqlite-journal"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
            {
                fs::remove_file(entry.path())
                    .with_context(|| format!("removing staged {}", entry.path().display()))?;
            }
        }
    }
    Ok(())
}
