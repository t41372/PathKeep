//! Responsibilities: encrypted portable bundles and verified native import.
//! Not responsible for: legacy SQLCipher conversion, derived models or UI prompts.
//! Failure modes: wrong key, omitted agent/evidence data, derived leakage, tampered
//! files, partial install, fresh-root refusal or App Lock without its local secret.
//! Tests: migration::native_tests.
use super::{
    ApplyImportOptions, ExportManifest, ExportManifestFile, ExportedBundle, ImportExclusionNote,
    ImportPreview, ImportResult,
};
use super::{intelligence_originals, portable_artifacts};
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

/// The bundle format that carries an archive of each layout. A bundle's format says which stores
/// it holds, so an import knows what to require before it decrypts anything.
const FORMATS: [(u32, vault_layout::ArchiveLayout); 2] = [
    (2, vault_layout::ArchiveLayout::BrowsingV1),
    // Adds the agent-history evidence and read model.
    (3, vault_layout::ArchiveLayout::AgentHistoryV2),
];

fn format_of(layout: vault_layout::ArchiveLayout) -> Result<u32> {
    FORMATS
        .iter()
        .find(|(_, of)| *of == layout)
        .map(|(format, _)| *format)
        .with_context(|| format!("no move bundle format carries a {layout:?} archive"))
}

/// A format above the newest this build writes is a newer PathKeep's (`move-file-too-new`, which
/// the window turns into "update PathKeep"); any other it does not know is not a file it reads.
fn layout_of(format: u32) -> Result<vault_layout::ArchiveLayout> {
    if let Some((_, layout)) = FORMATS.iter().find(|(of, _)| *of == format) {
        return Ok(*layout);
    }
    let newest = FORMATS.iter().map(|(of, _)| *of).max().unwrap_or_default();
    if format > newest {
        anyhow::bail!(
            "move-file-too-new: the file has format {format}; this build reads up to {newest}"
        );
    }
    anyhow::bail!("unsupported bundle format {format}")
}
/// Settings that belong to this computer, not to the archive, so a moved archive takes them from
/// the computer it lands on: App Lock (its secret never leaves the source machine; a fresh
/// machine starts without one) and whether the password is remembered in this keychain (export
/// always writes it off, which would otherwise clear the keychain on every move).
pub fn keep_machine_settings(config: &mut AppConfig, here: &AppConfig) {
    config.agent_viewer.clone_from(&here.agent_viewer);
    config.capture_policy = here.capture_policy.clone();
    config.dismissed_agent_hints.clone_from(&here.dismissed_agent_hints);
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
    vault.recheck_root()?;
    vault.require_complete()?;
    validate_target(vault.paths(), target)?;
    cancel.check()?;
    let stage = maintenance::staging_root(vault.paths())?;
    let mut config = vault.config().clone();
    config.archive_mode = ArchiveMode::Encrypted;
    config.remember_database_key_in_keyring = false;
    config.capture_policy = Default::default();
    config.agent_viewer = Default::default();
    config.dismissed_agent_hints.clear();
    config.app_lock = Default::default();
    let key = maintenance::create_envelope(stage.path(), &config, Some(password))?.unwrap();
    let _producers = vault.pause_conversations();
    vault.verify_conversion_source(&cancel)?;
    let pause = vault.pause_store()?;
    maintenance::copy_databases(
        &pause,
        stage.path(),
        Some(&key),
        &cancel,
        progress,
        &vault_layout::portable_databases(vault.layout()).collect::<Vec<_>>(),
    )?;
    drop(pause);
    crate::config::save_copy_config(vault.paths(), stage.path(), &config)?;
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
    for (id, name) in vault_layout::portable_files(vault.layout()) {
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
    if vault.layout() != vault_layout::ArchiveLayout::BrowsingV1 {
        intelligence_originals::pack(vault, &sealed, &key, &cancel)?;
        super::add_file_to_zip_cancel(
            &mut zip,
            &sealed.join(intelligence_originals::ENTRY),
            intelligence_originals::ENTRY,
            options,
            &mut files,
            &cancel,
        )?;
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
        format_version: format_of(vault.layout())?,
        app_version: crate::APP_VERSION.into(),
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
            relative.parent() == Some(Path::new(vault_layout::MOVE_FILES_V3))
                && relative.components().count() > 1
                && !vault_layout::export_target_is_managed(&first),
            "export-target-is-managed: choose a location outside the archive or in exports/pathkeep/format-3/"
        );
    }
    Ok(())
}

/// Preview authenticates the file inventory before asking for the export password.
pub fn preview(paths: &ProjectPaths, bundle: &Path) -> Result<ImportPreview> {
    crate::admission::refuse_newer(&paths.app_root)?;
    crate::storage_volume::require_snapshot_volume(paths)?;
    let manifest = super::read_and_validate_manifest(bundle)?;
    let layout = layout_of(manifest.format_version)?;
    let newest_schema = crate::store_schema::schema_version(vault_store::Db::Archive);
    anyhow::ensure!(
        manifest.archive_schema_version <= newest_schema,
        "move-file-too-new: the file's archive has schema {}; this build reads up to \
         {newest_schema}",
        manifest.archive_schema_version
    );
    anyhow::ensure!(
        manifest.archive_schema_version >= 1,
        "unsupported Turso archive schema {}",
        manifest.archive_schema_version
    );
    for required in
        vault_layout::portable_files(layout).map(|(_, name)| name).chain([portable_artifacts::NAME])
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
            "将在此设备上重建搜索和分析索引。新版搬家文件还会带上自定义网站分类、搜索引擎规则和已存网页文字的位置。",
            "App Lock 的密码和设置保留在各自设备上；新设备需重新设置。",
            "自动备份排程需在此设备上重新安装。",
            "安全副本留在创建它们的那台电脑上；这台电脑的安全副本保持不变。",
        ],
        crate::LanguagePreference::ZhTw => [
            "會在這台裝置上重建搜尋與分析索引。新版搬家檔也會帶上自訂網站分類、搜尋引擎規則與已存網頁文字的位置。",
            "App Lock 的密碼和設定保留在各自裝置上；新裝置需重新設定。",
            "自動備份排程需在此裝置上重新安裝。",
            "安全副本留在做出它們的那台電腦上；這台電腦的安全副本維持原樣。",
        ],
        _ => [
            "Search and analysis indexes are rebuilt here. New move files also carry your site categories, search rules and saved page text locations.",
            "App Lock secrets and settings stay on each machine; configure it again on a new machine.",
            "Install the automatic backup schedule again on this machine.",
            "Safety copies stay on the computer that made them; the ones on this computer stay as they are.",
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
                    vault_layout::ImportExclusion::SafetyCopies => 3,
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
    crate::admission::refuse_newer(&paths.app_root)?;
    crate::maintenance::rollback::require_resolved(&paths.app_root)?;
    if let Some(vault) = current {
        vault.require_complete()?;
    }
    // A delete-all-data the user confirmed has not finished. Installing an archive now would put
    // it where that wipe, once finished, deletes it.
    anyhow::ensure!(
        !crate::data_wipe_interrupted(paths),
        "archive-wipe-pending: a delete-all-data has not finished; it finishes before anything is \
         restored or imported"
    );
    // The bundle's own control is decided before the target gets a lock or a staging folder.
    if let Ok(key) = bundle_key(bundle, password) {
        crate::config::decode_config(bundle, &source_control(bundle, &key)?)?;
    }
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
    for (id, entry) in vault_layout::portable_files(layout_of(preview.manifest.format_version)?) {
        let descriptor = vault_layout::spec(id);
        if matches!(descriptor.lifecycle.portable, PortablePolicy::Database { .. })
            && entry != descriptor.path.relative()
        {
            let destination = stage.path().join(descriptor.path.relative());
            fs::create_dir_all(destination.parent().unwrap())?;
            let source = stage.path().join(entry);
            vault_fs::rename(&source, &destination)?;
        }
    }
    let mut config = crate::load_config(&staged_paths)?;
    config.archive_mode = ArchiveMode::Encrypted;
    // Taken again when the archive is installed; here for the stage, which opens below.
    keep_machine_settings(&mut config, &settings_here(paths)?);
    // The stamp of the archive it replaces, or a newer compatible one the bundle carries; a
    // bundle of the released layout then gets the stores it lacks, empty, when the stage opens.
    let stamp = crate::config::replacement_compatibility(paths, stage.path())?;
    crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
    let staged = Vault::open_staged(staged_paths, config.clone(), Some(&key))?;
    if preview.manifest.files.iter().any(|f| f.path == intelligence_originals::ENTRY) {
        intelligence_originals::unpack(&staged, stage.path(), &key, &cancel)?;
        fs::remove_file(stage.path().join(intelligence_originals::ENTRY))?;
    }
    staged.renew_browsing_run_origin()?;
    let layout = staged.layout();
    for db in vault_layout::portable_databases(layout) {
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
        kept_previous_as_copy: installed.previous_files.is_dir(),
        previous_copy_unchecked: installed.previous_unsealed.clone(),
    };
    Ok((report, installed))
}

/// The settings of this machine an import or a restore keeps (`keep_machine_settings`). Both
/// replace the archive, also one whose control is damaged or lost (a newer one is refused before
/// this). Then there are no settings of this machine to keep. A control that is there and could
/// not be read is a different matter and stops the operation.
pub(crate) fn settings_here(paths: &ProjectPaths) -> Result<AppConfig> {
    match crate::load_config(paths) {
        Ok(here) => Ok(here),
        Err(error) => match error.downcast_ref::<crate::admission::Refusal>() {
            Some(refusal) if refusal.reason != crate::admission::Reason::ControlUnreadable => {
                Ok(AppConfig::default())
            }
            _ => Err(error),
        },
    }
}

/// Puts a verified, closed staging root in place of this machine's archive and opens it.
/// Shared with the converter's import of v0.3 bundles; the caller holds the operation gate.
pub fn install_staged(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    owner: Arc<vault_store::OwnerLock>,
    stage: &Path,
    mut config: crate::AppConfig,
    key: Option<vault_crypto::DataKey>,
    cancel: &CancelToken,
) -> Result<InstalledArchive> {
    crate::storage_volume::require_snapshot_volume(paths)?;
    cancel.check()?;
    let _backup = current.map(Vault::begin_backup).transpose()?;
    let _producers = current.map(Vault::pause_conversations);
    let stamp = crate::config::replacement_compatibility(paths, stage)?;
    crate::config::write_staged_control(
        &crate::project_paths_with_root(stage),
        &config,
        Some(&stamp),
    )?;
    let combined =
        super::conversations::combine(paths, current, stage, &config, key.as_ref(), cancel)?;
    // The replacement's control is written and installed under one hold of the control's gate,
    // with this machine's settings as they are now: one changed while the import ran is kept.
    let control = super::import_policy::for_install(
        paths,
        current,
        combined.incoming.as_ref().map_or(stage, |d| d.path()),
        &mut config,
        key.as_ref(),
    )?;
    drop(combined.incoming);
    // Whoever built the stage, it replaces this archive: it keeps this archive's stamp unless its
    // own is newer and lowers neither floor.
    let stamp = crate::config::replacement_compatibility(paths, stage)?;
    let staged_paths = crate::project_paths_with_root(stage);
    crate::config::write_staged_control(&staged_paths, &config, Some(&stamp))?;
    if let Some(vault) = current {
        vault.pause_store()?.close()?;
    }
    // What the stage holds, by its own control, not by this build's newest layout. Read with the
    // archive closed: no backup adds a browser's history file to `raw-snapshots/` from here on.
    let layout = crate::admission::root_layout(stage)?;
    let mut files = vault_layout::portable_installation_files(layout);
    for id in vault_layout::portable_installation_by_entry(layout) {
        match id {
            A::RawSnapshots => {
                files.extend(super::safety_copies::installation(&paths.app_root, stage)?)
            }
            other => anyhow::bail!("an import has no rule for replacing {other:?} entry by entry"),
        }
    }
    // The archive it replaces becomes a safety copy, so the import can be undone.
    let replaced = maintenance::install::install(
        &control,
        stage,
        files,
        maintenance::install::Keep::SafetyCopy(maintenance::install::Before::Import),
    )?;
    drop(control);
    let vault = Vault::open_owned(paths.clone(), config, key.as_ref(), Some(owner))?;
    if replaced.kept.is_dir() {
        super::conversations::bind_previous(&vault, combined.batch_id, &replaced.kept)?;
    }
    Ok(InstalledArchive {
        vault,
        data_key: key,
        previous_files: replaced.kept,
        previous_unsealed: replaced.unsealed,
    })
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

/// The bundle's `config.json`, decrypted in the system temporary folder and returned in memory.
pub(super) fn source_control(bundle: &Path, key: &vault_crypto::DataKey) -> Result<Vec<u8>> {
    let entry = vault_layout::portable_entry(A::Config);
    let scratch = tempfile::tempdir()?;
    let sealed = scratch.path().join(entry);
    fs::create_dir_all(sealed.parent().context("bundle config entry has no parent")?)?;
    let mut zip = zip::ZipArchive::new(fs::File::open(bundle)?)?;
    std::io::copy(&mut zip.by_name(entry)?, &mut fs::File::create(&sealed)?)?;
    let mut control = Vec::new();
    vault_crypto::decrypt_artifact(scratch.path(), entry, key, &mut control, || false)?;
    Ok(control)
}

pub(super) fn bundle_key(bundle: &Path, password: &str) -> Result<vault_crypto::DataKey> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(fs::File::open(bundle)?)?;
    let entry = zip.by_name(vault_layout::portable_entry(A::Envelope))?;
    let mut bytes = Vec::new();
    entry.take((vault_crypto::ENVELOPE_LEN + 1) as u64).read_to_end(&mut bytes)?;
    crate::vault::password::unwrap(&bytes, password)
}

/// Password-free preview can describe visibility without publishing an encrypted
/// bundle's conversation inventory. Installation reads the same destination settings.
pub fn imported_agent_state_if_present(paths: &ProjectPaths) -> Result<crate::KindState> {
    crate::admission::refuse_newer(&paths.app_root)?;
    let config = settings_here(paths)?;
    crate::ensure_app_lock_unlocked(paths, &config)?;
    Ok(config.capture_policy.agent_state_if_present(true))
}
