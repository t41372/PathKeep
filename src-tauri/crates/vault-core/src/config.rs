//! Responsibilities: expose typed catalog-backed paths and persist the existing app config.
//! Not responsible for: declaring lifecycle membership, opening stores or changing disk formats.

use crate::admission::{Compatibility, Control, Reason, Refusal};
use crate::models::{AppConfig, normalize_app_config};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_layout::ArtifactId as A;

mod control_gate;
mod installation;
pub use crate::ai::model_download::reconcile_model_download_after_control_replacement;
pub use control_gate::ControlGate;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Canonical absolute paths for all backend-managed project artifacts.
pub struct ProjectPaths {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    /// The archive database (`archive/history-vault.db`), as `vault_store::Paths` lays it out.
    pub archive_db: PathBuf,
    /// Source evidence (`archive/source-evidence.db`).
    pub source_evidence_db: PathBuf,
    pub derived_dir: PathBuf,
    /// The full-text search index, a folder of segments (`derived/search-index`).
    pub search_index_dir: PathBuf,
    /// Insights and the queues (`derived/intelligence.db`).
    pub intelligence_db: PathBuf,
    pub audit_repo_path: PathBuf,
    pub manifests_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub raw_snapshots_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub quarantine_dir: PathBuf,
    pub schedule_dir: PathBuf,
    pub sidecars_dir: PathBuf,
    pub semantic_index_dir: PathBuf,
    pub intelligence_blobs_dir: PathBuf,
    /// AI vector sidecar plane (`derived/vectors/`); rebuildable, rides export via `derived/`.
    pub vectors_dir: PathBuf,
    /// Ask's conversations (`derived/agent.db`).
    pub agent_db: PathBuf,
    /// The v0.3 files an archive may still be in, read only to convert it.
    pub legacy: LegacyPaths,
    /// Downloaded embedding/reranker models (`<root>/models/`); re-downloadable, NOT exported.
    pub models_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub rust_log_path: PathBuf,
    pub frontend_log_path: PathBuf,
    pub crash_reports_dir: PathBuf,
    pub rust_panic_report_path: PathBuf,
    pub frontend_error_report_path: PathBuf,
}

/// The SQLCipher databases of PathKeep 0.3, under the same root. Only finding an archive that
/// still has to be converted and the conversion itself (`vault-convert-sqlcipher`) read them;
/// everything else uses the v0.4 files in [`ProjectPaths`]. They used to share field names with
/// those, which `Vault::open` rewrote, so a reader that did not go through it got a file that no
/// longer exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyPaths {
    pub archive: PathBuf,
    pub source_evidence: PathBuf,
    pub search: PathBuf,
    pub intelligence: PathBuf,
    pub agent: PathBuf,
}

impl ProjectPaths {
    /// Startup identification intentionally probes only the catalog's native primary.
    pub fn native_archive_path(&self) -> PathBuf {
        self.artifact_path(vault_layout::db_artifact(vault_layout::primary_database()))
    }

    /// Native copy discovery includes the released 0.5 folder and both 0.6 families.
    pub fn native_copy_folders(&self) -> [PathBuf; 3] {
        let older = self.artifact_path(A::NativeSnapshots);
        [
            older.clone(),
            older.join("v06"),
            self.app_root.join(vault_layout::PREVIOUS_GENERATIONS_V06),
        ]
    }

    /// Old-format recovery keeps its own primary instead of borrowing the native role set.
    pub fn legacy_archive_path(&self) -> PathBuf {
        self.artifact_path(vault_layout::legacy_primary())
    }

    /// Operation projections honor the existing caller-configurable typed paths. Artifacts
    /// without an exposed override use their catalog location under the supplied root.
    pub fn artifact_path(&self, id: A) -> PathBuf {
        match id {
            A::Root => self.app_root.clone(),
            A::History => self.archive_db.clone(),
            A::SourceEvidence => self.source_evidence_db.clone(),
            A::Intelligence => self.intelligence_db.clone(),
            A::Assistant => self.agent_db.clone(),
            A::Config => self.config_path.clone(),
            A::DerivedRoot => self.derived_dir.clone(),
            A::Search => self.search_index_dir.clone(),
            A::Audit => self.audit_repo_path.clone(),
            A::Manifests => self.manifests_dir.clone(),
            A::Exports => self.exports_dir.clone(),
            A::RawSnapshots => self.raw_snapshots_dir.clone(),
            A::Staging => self.staging_dir.clone(),
            A::Quarantine => self.quarantine_dir.clone(),
            A::Schedule => self.schedule_dir.clone(),
            A::Sidecars => self.sidecars_dir.clone(),
            A::LegacySemantic => self.semantic_index_dir.clone(),
            A::TextBlobs => self.intelligence_blobs_dir.clone(),
            A::Vectors => self.vectors_dir.clone(),
            A::Models => self.models_dir.clone(),
            A::Logs => self.logs_dir.clone(),
            A::CrashReports => self.crash_reports_dir.clone(),
            A::RustLog => self.rust_log_path.clone(),
            A::FrontendLog => self.frontend_log_path.clone(),
            A::RustPanic => self.rust_panic_report_path.clone(),
            A::FrontendError => self.frontend_error_report_path.clone(),
            A::LegacyHistory => self.legacy.archive.clone(),
            A::LegacyEvidence => self.legacy.source_evidence.clone(),
            A::LegacyIntelligence => self.legacy.intelligence.clone(),
            A::LegacySearch => self.legacy.search.clone(),
            A::LegacyAssistant => self.legacy.agent.clone(),
            A::ArchiveRoot => self.archive_db.parent().expect("archive parent").to_owned(),
            A::NativeSnapshots | A::LegacySnapshots => self.raw_snapshots_dir.join(
                Path::new(vault_layout::spec(id).path.relative())
                    .strip_prefix(vault_layout::spec(A::RawSnapshots).path.relative())
                    .unwrap(),
            ),
            A::MaintenanceStages => self.staging_dir.clone(),
            A::ScheduleLogs | A::ScheduleAttempts => self.schedule_dir.join(
                Path::new(vault_layout::spec(id).path.relative())
                    .strip_prefix(vault_layout::spec(A::Schedule).path.relative())
                    .unwrap(),
            ),
            A::SchedulerAudit => self.audit_repo_path.join(
                Path::new(vault_layout::spec(id).path.relative())
                    .strip_prefix(vault_layout::spec(A::Audit).path.relative())
                    .unwrap(),
            ),
            _ => vault_layout::path(&self.app_root, id),
        }
    }
}

/// Resolves the current project paths using the active project root.
pub fn project_paths() -> Result<ProjectPaths> {
    let root = project_root()?;
    Ok(project_paths_with_root(&root))
}

/// Builds the full project path layout for a specific root directory.
pub fn project_paths_with_root(root: &Path) -> ProjectPaths {
    let root = root.to_path_buf();
    let at = |id| vault_layout::path(&root, id);
    let logs_dir = at(A::Logs);
    let crash_reports_dir = at(A::CrashReports);
    let derived_dir = at(A::DerivedRoot);
    let sidecars_dir = at(A::Sidecars);
    let store = vault_store::Paths::under(&root);
    ProjectPaths {
        config_path: at(A::Config),
        archive_db: store.archive,
        source_evidence_db: store.source_evidence,
        derived_dir: derived_dir.clone(),
        search_index_dir: at(A::Search),
        intelligence_db: store.intelligence,
        audit_repo_path: at(A::Audit),
        manifests_dir: at(A::Manifests),
        exports_dir: at(A::Exports),
        raw_snapshots_dir: at(A::RawSnapshots),
        staging_dir: at(A::Staging),
        quarantine_dir: at(A::Quarantine),
        schedule_dir: at(A::Schedule),
        sidecars_dir: sidecars_dir.clone(),
        semantic_index_dir: at(A::LegacySemantic),
        intelligence_blobs_dir: at(A::TextBlobs),
        vectors_dir: at(A::Vectors),
        agent_db: store.agent,
        legacy: LegacyPaths {
            archive: at(A::LegacyHistory),
            source_evidence: at(A::LegacyEvidence),
            search: at(A::LegacySearch),
            intelligence: at(A::LegacyIntelligence),
            agent: at(A::LegacyAssistant),
        },
        models_dir: at(A::Models),
        logs_dir: logs_dir.clone(),
        rust_log_path: at(A::RustLog),
        frontend_log_path: at(A::FrontendLog),
        crash_reports_dir: crash_reports_dir.clone(),
        rust_panic_report_path: at(A::RustPanic),
        frontend_error_report_path: at(A::FrontendError),
        app_root: root,
    }
}

/// Resolves the active project root, honoring test and development overrides.
fn project_root() -> Result<PathBuf> {
    Ok(vault_build::resolve_root(
        vault_build::CURRENT,
        std::env::var_os(vault_build::ROOT_OVERRIDE_ENV),
        |identity| {
            ProjectDirs::from(identity.qualifier, identity.organization, identity.application)
                .map(|dirs| dirs.data_local_dir().to_path_buf())
        },
    )?)
}

/// Ensures the expected project directory layout exists on disk.
///
/// This is the first thing most operations do to a root, so it is also where a root this build
/// must not change is turned away: see [`crate::admission::guard`].
pub fn ensure_paths(paths: &ProjectPaths) -> Result<()> {
    crate::admission::guard(&paths.app_root)?;
    create_layout(paths)
}

fn create_layout(paths: &ProjectPaths) -> Result<()> {
    let directories: Vec<_> = vault_layout::scaffolding()
        .into_iter()
        .map(|(id, parent)| {
            let path = paths.artifact_path(id);
            if parent { path.parent().expect("database parent").to_owned() } else { path }
        })
        .collect();
    for directory in directories {
        fs::create_dir_all(&directory)
            .with_context(|| format!("creating {}", directory.display()))?;
    }
    Ok(())
}

/// What `config.json` holds: the settings, and the compatibility stamp a newer build may have
/// put around `archiveMode`. The settings never carry the stamp, so a caller that edits and
/// saves them cannot drop or alter it.
#[derive(Debug, Clone)]
pub struct ConfigDocument {
    pub config: AppConfig,
    pub compatibility: Option<Compatibility>,
}

/// Decodes a control file of a root this build may use. `archiveMode` is either the bare value
/// v0.4.0 and v0.5.0 write or the stamped object; everything else is the settings' own serde,
/// unchanged.
pub fn decode_config(root: &Path, bytes: &[u8]) -> Result<ConfigDocument, Refusal> {
    let control = crate::admission::parse_control(root, bytes)?;
    let refusal = |(reason, detail)| Refusal { reason, root: root.to_owned(), stamp: None, detail };
    let mut document = decode_settings(bytes, &control).map_err(refusal)?;
    if installation::has_choices(&document.config) {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Binding {
            capture_installation: Option<String>,
        }
        let binding: Binding = serde_json::from_slice(bytes)
            .map_err(|e| refusal((Reason::ControlInvalid, e.to_string())))?;
        if binding.capture_installation.is_none()
            || binding.capture_installation
                != installation::id(false)
                    .map_err(|e| refusal((Reason::ControlUnreadable, e.to_string())))?
        {
            document.config.capture_policy = Default::default();
            document.config.agent_viewer = Default::default();
            document.config.dismissed_agent_hints.clear();
        }
    }
    Ok(document)
}

/// Whether the settings around an already parsed `archiveMode` decode. See `admission::guard`.
pub(crate) fn check_settings(root: &Path, bytes: &[u8], control: &Control) -> Result<(), Refusal> {
    decode_settings(bytes, control).map(|_| ()).map_err(|(reason, detail)| Refusal {
        reason,
        root: root.to_owned(),
        stamp: None,
        detail,
    })
}

/// The codec alone: reads a control whatever its stamp says. This is how a stamp is carried from
/// one file to another. It is not a decision that the root it came from may be used.
pub fn decode_document(bytes: &[u8]) -> Result<ConfigDocument> {
    let failed =
        |(reason, detail): (Reason, String)| anyhow::anyhow!("{}: {detail}", reason.code());
    let control = crate::admission::parse_envelope(bytes).map_err(failed)?;
    decode_settings(bytes, &control).map_err(failed)
}

fn decode_settings(bytes: &[u8], control: &Control) -> Result<ConfigDocument, (Reason, String)> {
    let invalid = |error: serde_json::Error| {
        (Reason::ControlInvalid, format!("parsing config json: {error}"))
    };
    reject_duplicate_keys(bytes).map_err(invalid)?;
    let mut config = match &control.stamped_mode {
        None => serde_json::from_slice::<AppConfig>(bytes).map_err(invalid)?,
        Some(mode) => {
            let mut value: serde_json::Value = serde_json::from_slice(bytes).map_err(invalid)?;
            value["archiveMode"] = serde_json::from_str(mode.get()).map_err(invalid)?;
            serde_json::from_value(value).map_err(invalid)?
        }
    };
    config.capture_policy.validate().map_err(|e| (Reason::ControlInvalid, e.to_string()))?;
    normalize_app_config(&mut config);
    Ok(ConfigDocument { config, compatibility: control.compatibility.clone() })
}

/// No PathKeep writes a key twice, and two readers can take different copies of one, so a control
/// with a key twice in any object is damaged. The derived decoders refuse a field they see twice,
/// but a stamped control passes through a generic JSON value first, which keeps the last copy.
fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), serde_json::Error> {
    use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
    struct Unique;
    impl<'de> DeserializeSeed<'de> for Unique {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
            deserializer.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Unique {
        type Value = ();
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a JSON value")
        }
        fn visit_bool<E>(self, _: bool) -> Result<(), E> {
            Ok(())
        }
        fn visit_i64<E>(self, _: i64) -> Result<(), E> {
            Ok(())
        }
        fn visit_u64<E>(self, _: u64) -> Result<(), E> {
            Ok(())
        }
        fn visit_f64<E>(self, _: f64) -> Result<(), E> {
            Ok(())
        }
        fn visit_str<E>(self, _: &str) -> Result<(), E> {
            Ok(())
        }
        fn visit_unit<E>(self) -> Result<(), E> {
            Ok(())
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<(), A::Error> {
            while items.next_element_seed(Unique)?.is_some() {}
            Ok(())
        }
        fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<(), A::Error> {
            let mut seen = std::collections::HashSet::new();
            while let Some(key) = entries.next_key::<String>()? {
                if !seen.insert(key.clone()) {
                    return Err(A::Error::custom(format!("the key `{key}` appears twice")));
                }
                entries.next_value_seed(Unique)?;
            }
            Ok(())
        }
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    Unique.deserialize(&mut deserializer)?;
    deserializer.end()
}
#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredSettings<'a> {
    #[serde(flatten)]
    config: &'a AppConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    capture_installation: Option<String>,
}

/// Encodes settings for disk. Without a stamp this is byte for byte what v0.5.0 writes and
/// reads. With one, the stamp's bytes are written exactly as they were read.
pub fn encode_config(config: &AppConfig, compatibility: Option<&Compatibility>) -> Result<String> {
    config.capture_policy.validate()?;
    let binding = if installation::has_choices(config) { installation::id(true)? } else { None };
    let stored = StoredSettings { config, capture_installation: binding };
    let serde_json::Value::Object(fields) = serde_json::to_value(stored)? else {
        anyhow::bail!("settings must be an object")
    };
    let Some(compatibility) = compatibility else {
        return Ok(serde_json::to_string_pretty(&fields)?);
    };
    #[derive(Serialize)]
    struct StampedMode<'a> {
        #[serde(rename = "formatVersion")]
        format_version: u64,
        mode: &'a crate::ArchiveMode,
        compatibility: &'a serde_json::value::RawValue,
    }
    struct Stamped<'a> {
        fields: serde_json::Map<String, serde_json::Value>,
        mode: StampedMode<'a>,
    }
    impl Serialize for Stamped<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut map = serializer.serialize_map(Some(self.fields.len()))?;
            for (key, value) in &self.fields {
                if key == "archiveMode" {
                    map.serialize_entry(key, &self.mode)?;
                } else {
                    map.serialize_entry(key, value)?;
                }
            }
            map.end()
        }
    }
    anyhow::ensure!(fields.contains_key("archiveMode"), "the configuration has no archiveMode");
    Ok(serde_json::to_string_pretty(&Stamped {
        fields,
        mode: StampedMode {
            format_version: 2,
            mode: &config.archive_mode,
            compatibility: compatibility.raw(),
        },
    })?)
}

/// Reads the control file of a root this build may use.
///
/// Fails with a [`Refusal`] when it may not: the archive needs a newer PathKeep, the control is
/// damaged, or it is missing next to data (which is never read as a first run). A root with no
/// control and no archive in it yields the defaults, not initialized.
pub fn load_document(paths: &ProjectPaths) -> Result<ConfigDocument> {
    let root = &paths.app_root;
    loop {
        if let Some(bytes) = crate::admission::read_control(root)? {
            return Ok(decode_config(root, &bytes)?);
        }
        // No control. If one lands between that look and this one (an install finishing), the
        // answer is that control, so look again.
        if crate::admission::require_control(root)?.is_none() {
            let mut config = AppConfig::default();
            normalize_app_config(&mut config);
            return Ok(ConfigDocument { config, compatibility: None });
        }
    }
}

/// Loads the persisted app config, normalizing newly added runtime defaults. See
/// [`load_document`] for when it refuses.
pub fn load_config(paths: &ProjectPaths) -> Result<AppConfig> {
    Ok(load_document(paths)?.config)
}

/// Saves a live archive's settings. The gate is the proof that this is the second half of a
/// read-modify-write: the writer took it, read the control, changed its own fields and writes the
/// result here (see `control_gate`). A stage or a copy that is not live yet is written with
/// [`write_staged_control`] instead.
///
/// Persists through [`crate::durable_io::atomic_durable_write`] (temp -> F_FULLFSYNC
/// -> rename -> dir fsync) rather than a bare `fs::write`, so a crash mid-save leaves
/// `config.json` as EITHER the complete old config OR the complete new one — never the
/// truncated/empty file that silently dropped a mode change in the 2026-06-30 incident.
/// The layout is created first: the durable write renames a sibling temp onto the destination
/// and does not create parents.
///
/// The control on disk is read before anything is created. One this build may not use stops the
/// save; a compatibility stamp in it is written back exactly as it was, whatever `config` says.
/// Except where setup writes the first control of an archive over one that was not initialized
/// (what "Delete all data" leaves if it stops just before its end): that stamp described no
/// archive, and a new one is stamped once its stores exist (`maintenance::creation`). Kept, it
/// would say the stores of its revision are there, and the new archive would not open.
pub fn save_config(gate: &ControlGate, config: &AppConfig) -> Result<()> {
    save_config_with_durability_warning(gate, config).map(drop)
}

/// A successful save can carry a directory-sync warning after the new control is installed.
pub fn save_config_with_durability_warning(
    gate: &ControlGate,
    config: &AppConfig,
) -> Result<Option<String>> {
    let paths = gate.paths();
    let compatibility =
        crate::admission::guard(&paths.app_root)?.and_then(|control| control.compatibility);
    let compatibility = match compatibility {
        Some(_) if config.initialized && !load_document(paths)?.config.initialized => None,
        stamp => stamp,
    };
    create_layout(paths)?;
    let content = encode_config(config, compatibility.as_ref())?;
    let warning = match crate::durable_io::write_live_config(&paths.config_path, content.as_bytes())
    {
        Ok(()) => None,
        Err(error) if error.is::<crate::durable_io::PublishedButUnsynced>() => {
            let warning = format!("{}: {error:#}", paths.config_path.display());
            eprintln!("pathkeep: {warning}");
            Some(warning)
        }
        Err(error) => {
            return Err(error).with_context(|| format!("writing {}", paths.config_path.display()));
        }
    };
    if !config.ai.wants_builtin_model() {
        crate::ai::cancel_archive_model_downloads(paths);
    }
    Ok(warning)
}

/// Writes `config` with `compatibility` into a stage: an archive being built that is not the live
/// one yet. Its own control is not consulted; the operation building it decides the stamp.
pub fn write_staged_control(
    paths: &ProjectPaths,
    config: &AppConfig,
    compatibility: Option<&Compatibility>,
) -> Result<()> {
    create_layout(paths)?;
    let content = encode_config(config, compatibility)?;
    crate::durable_io::atomic_durable_write(&paths.config_path, content.as_bytes())
        .with_context(|| format!("writing {}", paths.config_path.display()))?;
    Ok(())
}

/// The stamp a stage that replaces the live archive gets: see `admission::compose`. The live
/// archive's stamp counts only when its control can be read; a damaged one is what the operation
/// repairs.
pub(crate) fn replacement_compatibility(
    live: &ProjectPaths,
    stage: &Path,
) -> Result<Compatibility> {
    let target = crate::admission::guard(&live.app_root)
        .ok()
        .flatten()
        .and_then(|control| control.compatibility);
    let source = crate::admission::guard(stage)?.and_then(|control| control.compatibility);
    crate::admission::compose(target.as_ref(), source.as_ref()).map_err(anyhow::Error::msg)
}

/// Writes `config` into a staged or snapshot copy of `live`'s archive, with the compatibility
/// stamp of `live`: a copy of a root is the same format as the root, whatever settings change.
pub(crate) fn save_copy_config(live: &ProjectPaths, copy: &Path, config: &AppConfig) -> Result<()> {
    let compatibility =
        crate::admission::guard(&live.app_root)?.and_then(|control| control.compatibility);
    let paths = project_paths_with_root(copy);
    create_layout(&paths)?;
    let content = encode_config(config, compatibility.as_ref())?;
    crate::durable_io::atomic_durable_write(&paths.config_path, content.as_bytes())
        .with_context(|| format!("writing {}", paths.config_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(debug_assertions)]
    #[test]
    fn config_parent_sync_checkpoint_only_warns_for_the_installed_live_control() {
        let dir = tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig { menu_bar_icon: true, ..Default::default() };
        let _fault = crate::fault_inject::FaultGuard::error_at("config-save-parent-sync");
        let stage = project_paths_with_root(&dir.path().join("copy"));
        write_staged_control(&stage, &config, None).expect("stage sync succeeds");
        let gate = ControlGate::acquire(&paths);
        let warning = save_config_with_durability_warning(&gate, &config).unwrap().unwrap();
        assert!(warning.contains("config-save-parent-sync"));
        assert!(load_config(&paths).unwrap().menu_bar_icon);
        assert!(save_config_with_durability_warning(&gate, &config).unwrap().is_none());
    }

    #[test]
    fn load_config_returns_defaults_when_missing_and_errors_on_invalid_json() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let default_config = load_config(&paths).expect("missing config should default");
        assert!(!default_config.initialized);

        fs::write(&paths.config_path, "{not-json").expect("write invalid config");
        let error = load_config(&paths).expect_err("invalid config should fail");
        assert!(error.to_string().contains("parsing config json"));
    }

    #[test]
    fn writing_a_control_cannot_publish_an_all_paused_policy() {
        let dir = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(dir.path());
        let gate = ControlGate::acquire(&paths);
        let mut config = AppConfig::default();
        save_config(&gate, &config).unwrap();
        let before = fs::read(&paths.config_path).unwrap();
        config.capture_policy.browsing = crate::KindState::Paused;
        config.capture_policy.agent_history = crate::KindState::Paused;
        assert_eq!(save_config(&gate, &config).unwrap_err().to_string(), "last-active-kind");
        assert_eq!(fs::read(&paths.config_path).unwrap(), before);
    }
}
