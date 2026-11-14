//! Responsibilities: encrypted portable bundles and verified native import.
//! Not responsible for: legacy SQLCipher conversion, derived models or UI prompts.
//! Failure modes: wrong key, omitted agent/evidence data, derived leakage, tampered
//! files, partial install, fresh-root refusal or App Lock without its local secret.
//! Tests: migration::native_tests.
use super::portable_artifacts;
use super::{
    ApplyImportOptions, ExportManifest, ExportManifestFile, ExportedBundle, ImportExclusionNote,
    ImportPreview, ImportResult,
};
use crate::{
    AppConfig, ArchiveMode, ProjectPaths, Vault,
    maintenance::{self, InstalledArchive, Progress},
};
use anyhow::{Context, Result};
use std::{
    fs::{self},
    io::Write,
    path::Path,
    sync::Arc,
};
use vault_layout::{ArtifactId as A, PortablePolicy};
use vault_store::CancelToken;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const VERSION: u32 = 2;
/// Settings that belong to this computer, not to the archive, so a moved archive takes them from
/// the computer it lands on: App Lock (its secret never leaves the source machine; a fresh
/// machine starts without one) and whether the password is remembered in this keychain (export
/// always writes it off, which would otherwise clear the keychain on every move).
pub fn keep_machine_settings(config: &mut AppConfig, here: &AppConfig) {
    config.app_lock = here.app_lock.clone();
    config.remember_database_key_in_keyring = here.remember_database_key_in_keyring;
}

/// Export always uses a fresh data key wrapped with the export password.
pub fn export(
    vault: &Vault,
    target: &Path,
    password: &str,
    cancel: CancelToken,
    progress: impl FnMut(Progress) + Send + 'static,
) -> Result<ExportedBundle> {
    let _operation = crate::archive::ArchiveOpGate::acquire(vault.paths());
    validate_target(vault.paths(), target)?;
    cancel.check()?;
    let stage = maintenance::staging_root(vault.paths())?;
    let mut config = vault.config().clone();
    config.archive_mode = ArchiveMode::Encrypted;
    config.remember_database_key_in_keyring = false;
    config.app_lock = Default::default();
    let key = maintenance::create_envelope(stage.path(), &config, Some(password))?.unwrap();
    let pause = vault.store().pause()?;
    maintenance::copy_databases(
        &pause,
        stage.path(),
        Some(&key),
        &cancel,
        progress,
        &vault_layout::portable_databases().collect::<Vec<_>>(),
    )?;
    drop(pause);
    crate::save_config(&crate::project_paths_with_root(stage.path()), &config)?;
    let parent = target.parent().context("bundle target needs a parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut zip = ZipWriter::new(temporary.reopen()?);
    let mut files = Vec::<ExportManifestFile>::new();
    let options =
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored).large_file(true);
    let sealed = stage.path().join("sealed");
    fs::create_dir_all(
        sealed.join(Path::new(vault_layout::portable_entry(A::Config)).parent().unwrap()),
    )?;
    vault_crypto::encrypt_artifact(
        &sealed,
        vault_layout::portable_entry(A::Config),
        &key,
        &mut fs::File::open(vault_layout::path(stage.path(), A::Config))?,
        || cancel.check().is_err(),
    )?;
    for (id, name) in vault_layout::portable_files() {
        cancel.check()?;
        let source = if matches!(
            vault_layout::spec(id).lifecycle.portable,
            PortablePolicy::SealedConfig { .. }
        ) {
            sealed.join(name)
        } else {
            stage.path().join(vault_layout::spec(id).path.relative())
        };
        super::add_file_to_zip_cancel(&mut zip, &source, name, options, &mut files, &cancel)?;
    }
    let artifacts = portable_artifacts::pack(vault.paths(), &sealed, &key, &cancel)?;
    super::add_file_to_zip_cancel(
        &mut zip,
        &artifacts,
        portable_artifacts::NAME,
        options,
        &mut files,
        &cancel,
    )?;
    let manifest = ExportManifest {
        format_version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        archive_schema_version: crate::store_schema::schema_version(vault_store::Db::Archive),
        archive_mode: "encrypted".into(),
        exported_at: crate::utils::now_rfc3339(),
        exporter_hostname: None,
        files,
    };
    let bytes = serde_json::to_vec(&manifest)?;
    zip.start_file(super::MANIFEST_ZIP_PATH, options)?;
    zip.write_all(&bytes)?;
    zip.start_file(super::MANIFEST_SHA_ZIP_PATH, options)?;
    zip.write_all(crate::utils::sha256_hex(&bytes).as_bytes())?;
    zip.finish()?.sync_all()?;
    cancel.check()?;
    crate::durable_io::install_file_durably(temporary.path(), target)?;
    Ok(ExportedBundle {
        bundle_path: target.into(),
        bytes_written: fs::metadata(target)?.len(),
        manifest,
    })
}

fn validate_target(paths: &ProjectPaths, target: &Path) -> Result<()> {
    let parent = target.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let target =
        fs::canonicalize(parent)?.join(target.file_name().context("export needs a file name")?);
    let root = fs::canonicalize(&paths.app_root)?;
    if let Ok(relative) = target.strip_prefix(root) {
        let first = relative
            .components()
            .next()
            .context("export-target-is-managed: choose a file outside the archive root")?
            .as_os_str()
            .to_string_lossy();
        anyhow::ensure!(
            !vault_layout::export_target_is_managed(&first),
            "export-target-is-managed: choose an export file outside the archive's managed data"
        );
    }
    Ok(())
}

/// Preview authenticates the file inventory before asking for the export password.
pub fn preview(paths: &ProjectPaths, bundle: &Path) -> Result<ImportPreview> {
    let manifest = super::read_and_validate_manifest(bundle)?;
    anyhow::ensure!(
        manifest.format_version == VERSION,
        "unsupported bundle format {}",
        manifest.format_version
    );
    anyhow::ensure!(
        (1..=crate::store_schema::schema_version(vault_store::Db::Archive))
            .contains(&manifest.archive_schema_version),
        "unsupported Turso archive schema {}",
        manifest.archive_schema_version
    );
    for required in
        vault_layout::portable_files().map(|(_, name)| name).chain([portable_artifacts::NAME])
    {
        anyhow::ensure!(
            manifest.files.iter().any(|f| f.path == required),
            "bundle is missing required entry {required}"
        );
    }
    for file in &manifest.files {
        super::validate_bundle_relative_path(&file.path)?;
    }
    Ok(ImportPreview {
        // An older archive opens and its migrations run, as when this app updates in place.
        schema_up_to_date: manifest.archive_schema_version
            == crate::store_schema::schema_version(vault_store::Db::Archive),
        migrations_to_apply: Vec::new(),
        bytes_to_extract: manifest.files.iter().map(|f| f.size_bytes).sum(),
        will_overwrite_existing: will_overwrite(paths),
        manifest,
        exclusion_notes: exclusion_notes(paths),
    })
}

/// Whether installing a bundle replaces an archive (native or still in the old format).
pub fn will_overwrite(paths: &ProjectPaths) -> bool {
    paths.native_archive_path().exists()
        || vault_layout::path(&paths.app_root, vault_layout::legacy_primary()).exists()
        || maintenance::install::pending(paths)
}

/// What a bundle doesn't bring, in the language of this machine.
pub fn exclusion_notes(paths: &ProjectPaths) -> Vec<ImportExclusionNote> {
    let copy = match crate::load_config(paths).unwrap_or_default().preferred_language {
        crate::LanguagePreference::ZhCn => [
            "搜索索引和智能分析数据将在此设备上重建。",
            "App Lock 的密码和设置保留在各自设备上；新设备需重新设置。",
            "自动备份排程需在此设备上重新安装。",
        ],
        crate::LanguagePreference::ZhTw => [
            "搜尋索引和智慧分析資料將在此裝置上重建。",
            "App Lock 的密碼和設定保留在各自裝置上；新裝置需重新設定。",
            "自動備份排程需在此裝置上重新安裝。",
        ],
        _ => [
            "Search and intelligence are rebuilt on this machine.",
            "App Lock secrets and settings stay on each machine; configure it again on a new machine.",
            "Install the automatic backup schedule again on this machine.",
        ],
    };
    vault_layout::import_exclusions()
        .into_iter()
        .map(|(id, reason)| {
            let descriptor = vault_layout::spec(id);
            let path = if matches!(
                descriptor.lifecycle.open,
                vault_layout::OpenPolicy::EnsureContainer(_)
            ) {
                format!("{}/", descriptor.path.relative())
            } else {
                descriptor.path.relative().to_owned()
            };
            ImportExclusionNote {
                path,
                reason: copy[match reason {
                    vault_layout::ImportExclusion::Rebuilt => 0,
                    vault_layout::ImportExclusion::MachineSecrets => 1,
                    vault_layout::ImportExclusion::Schedule => 2,
                }]
                .into(),
            }
        })
        .collect()
}

/// Verify an isolated copy, build search, then install under the owner's journal.
pub fn import(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    reservation: Option<Arc<vault_store::OwnerLock>>,
    bundle: &Path,
    options: &ApplyImportOptions,
    cancel: CancelToken,
) -> Result<(ImportResult, InstalledArchive)> {
    let _operation = crate::archive::ArchiveOpGate::acquire(paths);
    let password =
        options.source_archive_key.as_deref().context(super::IMPORT_SOURCE_KEY_REQUIRED_PREFIX)?;
    let owner = match current {
        Some(vault) => vault.store().owner()?,
        None => match reservation {
            Some(owner) => owner,
            None => Arc::new(vault_store::OwnerLock::acquire(&paths.app_root)?),
        },
    };
    maintenance::recover_native_maintenance(paths)?;
    let preview = preview(paths, bundle)?;
    anyhow::ensure!(
        options.confirm_overwrite || !preview.will_overwrite_existing,
        "confirmOverwrite is required"
    );
    let key = bundle_key(bundle, password).context(super::IMPORT_SOURCE_KEY_INVALID_PREFIX)?;
    let stage = maintenance::staging_root(paths)?;
    super::extract_bundle_into_cancel(bundle, stage.path(), &preview.manifest, &cancel)?;
    let staged_paths = crate::project_paths_with_root(stage.path());
    let origin = portable_artifacts::unpack(stage.path(), &key, &cancel)?;
    portable_artifacts::remember_origin(stage.path(), &origin.source_root)?;
    let mut config_file = fs::File::create(vault_layout::path(stage.path(), A::Config))?;
    vault_crypto::decrypt_artifact(
        stage.path(),
        vault_layout::portable_entry(A::Config),
        &key,
        &mut config_file,
        || cancel.check().is_err(),
    )?;
    config_file.sync_all()?;
    drop(config_file);
    for (id, entry) in vault_layout::portable_files() {
        let descriptor = vault_layout::spec(id);
        if matches!(descriptor.lifecycle.portable, PortablePolicy::Database { .. })
            && entry != descriptor.path.relative()
        {
            let destination = stage.path().join(descriptor.path.relative());
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::rename(stage.path().join(entry), destination)?;
        }
    }
    let mut config = crate::load_config(&staged_paths)?;
    config.archive_mode = ArchiveMode::Encrypted;
    keep_machine_settings(&mut config, &crate::load_config(paths)?);
    crate::save_config(&staged_paths, &config)?;
    let staged = Vault::open_staged(staged_paths, config.clone(), Some(&key))?;
    for db in vault_layout::portable_databases() {
        cancel.check()?;
        anyhow::ensure!(
            staged.store().read(db, |s| s.integrity_check())? == ["ok"],
            "bundle database integrity failed"
        );
    }
    portable_artifacts::relocate(&staged, &origin.source_root, &paths.app_root, &cancel)?;
    staged.search().rebuild_with_control(cancel.clone(), |_| {})?;
    staged.close()?;
    let installed =
        install_staged(paths, current, owner, stage.path(), config, Some(key), &cancel)?;
    let report = ImportResult {
        manifest: preview.manifest,
        migrations_applied: Vec::new(),
        final_schema_version: crate::store_schema::schema_version(vault_store::Db::Archive),
        preserved_previous_as_bak: preview.will_overwrite_existing,
    };
    Ok((report, installed))
}

/// Puts a verified, closed staging root in place of this machine's archive and opens it.
/// Shared with the converter's import of v0.3 bundles; the caller holds the operation gate.
pub fn install_staged(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    owner: Arc<vault_store::OwnerLock>,
    stage: &Path,
    config: crate::AppConfig,
    key: Option<vault_crypto::DataKey>,
    cancel: &CancelToken,
) -> Result<InstalledArchive> {
    cancel.check()?;
    if let Some(vault) = current {
        vault.store().pause()?.close()?;
        vault.close()?;
    }
    let files = vault_layout::portable_installation_files();
    let previous = maintenance::install::install(paths, stage, files, true)?;
    let vault = Vault::open_owned(paths.clone(), config, key.as_ref(), Some(owner))?;
    Ok(InstalledArchive { vault, data_key: key, previous_files: previous })
}

/// Paths the exporting machine stored (snapshots, audit files, provenance) point into its
/// own data folder; rewrite them to this one and remember the old root for audit records,
/// which keep their original text.
pub fn adopt_moved_paths(
    staged: &Vault,
    stage: &Path,
    source_root: &str,
    to: &Path,
    cancel: &CancelToken,
) -> Result<()> {
    portable_artifacts::remember_origin(stage, source_root)?;
    portable_artifacts::relocate(staged, source_root, to, cancel)
}

fn bundle_key(bundle: &Path, password: &str) -> Result<vault_crypto::DataKey> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(fs::File::open(bundle)?)?;
    let entry = zip.by_name(vault_layout::portable_entry(A::Envelope))?;
    let mut bytes = Vec::new();
    entry.take((vault_crypto::ENVELOPE_LEN + 1) as u64).read_to_end(&mut bytes)?;
    crate::vault::password::unwrap(&bytes, password)
}
