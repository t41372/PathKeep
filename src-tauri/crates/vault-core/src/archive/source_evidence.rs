//! Cold source-evidence storage for browser-native metadata.
//!
//! ## Responsibilities
//! - Own the archived `archive/source-evidence.sqlite` plane that preserves
//!   schema observations, capability snapshots, typed evidence, and native
//!   entities without polluting the hot canonical archive.
//! - Provide the post-commit persistence helpers used by backup/import flows
//!   after canonical archive rows have committed successfully.
//! - Offer a bounded-memory deferred payload contract so large source-evidence
//!   batches can spill to temporary disk before the cold archive write runs.
//!
//! ## Not responsible for
//! - Writing canonical URL/visit/download/search-term/favicon rows.
//! - Driving archive run-ledger, manifest, or checkpoint orchestration.
//! - Serving default shell queries; this plane remains a cold explain/debug
//!   boundary.
//!
//! ## Dependencies
//! - `browser_history_parser` read models for typed evidence and native
//!   entities.
//! - `tempfile` for spill-to-disk payloads that would otherwise keep large
//!   post-commit batches in RAM.
//!
//! ## Performance notes
//! - Canonical ingest stays on the hot archive path; source-evidence writes are
//!   deferred until after the canonical transaction commits.
//! - Large deferred payloads spill to temporary files in `staging/` so long
//!   imports do not retain every source-native JSON blob in memory until the
//!   cold archive catch-up write begins.

use crate::{
    archive::apply_cipher_key,
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveMode},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use browser_history_parser::{
    CapabilitySnapshot, NativeEntity, ParsedHistory, SchemaObservation, TypedEvidenceBatch,
};
use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader},
    time::Duration as StdDuration,
};
use tempfile::{Builder as TempFileBuilder, TempPath};

const SOURCE_EVIDENCE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS source_batches (
  id                       INTEGER PRIMARY KEY AUTOINCREMENT,
  source_profile_id        INTEGER NOT NULL,
  run_id                   INTEGER,
  source_kind              TEXT NOT NULL,
  browser_version          TEXT,
  schema_version_text      TEXT,
  schema_version_int       INTEGER,
  schema_fingerprint       TEXT NOT NULL,
  parser_version           TEXT NOT NULL,
  capability_snapshot_json TEXT NOT NULL,
  coverage_stats_json      TEXT NOT NULL,
  artifact_refs_json       TEXT,
  notes_json               TEXT,
  created_at               TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_observations (
  source_batch_id  INTEGER NOT NULL,
  database_label   TEXT NOT NULL,
  observation_json TEXT NOT NULL,
  recorded_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_search_evidence (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id   INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id   TEXT,
  source_url_id     TEXT,
  evidence_key      TEXT NOT NULL,
  evidence_value    TEXT NOT NULL,
  normalized_value  TEXT,
  source_field      TEXT NOT NULL,
  recorded_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_navigation_evidence (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id   INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id   TEXT NOT NULL,
  edge_kind         TEXT NOT NULL,
  target_visit_id   TEXT,
  target_url        TEXT,
  transition        INTEGER,
  source_field      TEXT NOT NULL,
  recorded_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_engagement_evidence (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id    INTEGER NOT NULL,
  source_profile_id  INTEGER NOT NULL,
  source_visit_id    TEXT NOT NULL,
  metric_key         TEXT NOT NULL,
  metric_value_int   INTEGER,
  metric_value_real  REAL,
  source_field       TEXT NOT NULL,
  recorded_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_context_evidence (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id   INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id   TEXT,
  source_url_id     TEXT,
  context_key       TEXT NOT NULL,
  value_json        TEXT NOT NULL,
  source_field      TEXT NOT NULL,
  recorded_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS native_entities (
  id                         INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id            INTEGER NOT NULL,
  source_profile_id          INTEGER NOT NULL,
  entity_kind                TEXT NOT NULL,
  native_primary_key         TEXT NOT NULL,
  parent_native_primary_key  TEXT,
  payload_json               TEXT NOT NULL,
  metadata_json              TEXT NOT NULL,
  recorded_at                TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_source_batches_profile_created
  ON source_batches(source_profile_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_source_batches_run
  ON source_batches(run_id);
CREATE INDEX IF NOT EXISTS idx_visit_search_evidence_batch
  ON visit_search_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_visit_navigation_evidence_batch
  ON visit_navigation_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_visit_engagement_evidence_batch
  ON visit_engagement_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_visit_context_evidence_batch
  ON visit_context_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_native_entities_batch_kind
  ON native_entities(source_batch_id, entity_kind);
"#;

pub(crate) const SOURCE_EVIDENCE_SPOOL_THRESHOLD_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct SourceBatchInput {
    pub source_profile_id: i64,
    pub run_id: Option<i64>,
    pub source_kind: String,
    pub browser_version: Option<String>,
    pub schema_version_text: Option<String>,
    pub schema_version_int: Option<i64>,
    pub schema_fingerprint: String,
    pub capability_snapshot: CapabilitySnapshot,
    pub coverage_stats_json: String,
    pub artifact_refs_json: Option<String>,
    pub notes_json: Option<String>,
}

/// Cold evidence payload that still needs to be written after canonical ingest commits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SourceEvidencePayload {
    pub typed_evidence: TypedEvidenceBatch,
    pub native_entities: Vec<NativeEntity>,
}

/// Defers one cold payload until the archive transaction has committed.
///
/// Small payloads stay in memory. Larger ones spill into a temporary JSON file
/// under `staging/source-evidence-spool/` so import/backup flows do not keep
/// every native payload resident until post-commit source-evidence persistence.
#[derive(Debug)]
pub(crate) enum DeferredSourceEvidencePayload {
    InMemory(SourceEvidencePayload),
    SpoolFile(TempPath),
    ChunkFile(TempPath),
}

impl DeferredSourceEvidencePayload {
    /// Returns `true` when this payload has been spilled to disk.
    #[cfg(test)]
    pub(crate) fn is_spooled(&self) -> bool {
        matches!(self, Self::SpoolFile(_))
    }

    /// Persists the deferred payload into the cold source-evidence archive.
    pub(crate) fn persist(
        &self,
        transaction: &Transaction<'_>,
        source_batch_id: i64,
        source_profile_id: i64,
    ) -> Result<()> {
        match self {
            Self::InMemory(payload) => {
                persist_source_evidence(transaction, source_batch_id, source_profile_id, payload)
            }
            Self::SpoolFile(path) => {
                let file = fs::File::open(path).with_context(|| {
                    format!("opening deferred source-evidence {}", path.display())
                })?;
                let payload: SourceEvidencePayload =
                    serde_json::from_reader(file).with_context(|| {
                        format!("reading deferred source-evidence {}", path.display())
                    })?;
                persist_source_evidence(transaction, source_batch_id, source_profile_id, &payload)
            }
            Self::ChunkFile(path) => {
                let file = fs::File::open(path).with_context(|| {
                    format!("opening deferred source-evidence chunk file {}", path.display())
                })?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = line.with_context(|| {
                        format!("reading deferred source-evidence chunk {}", path.display())
                    })?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let payload: SourceEvidencePayload =
                        serde_json::from_str(&line).with_context(|| {
                            format!(
                                "deserializing deferred source-evidence chunk {}",
                                path.display()
                            )
                        })?;
                    persist_source_evidence(
                        transaction,
                        source_batch_id,
                        source_profile_id,
                        &payload,
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// Moves a post-commit payload into bounded storage before the cold archive write begins.
pub(crate) fn defer_source_evidence_payload(
    paths: &ProjectPaths,
    label: &str,
    payload: SourceEvidencePayload,
) -> Result<DeferredSourceEvidencePayload> {
    if approx_source_evidence_payload_bytes(&payload) <= SOURCE_EVIDENCE_SPOOL_THRESHOLD_BYTES {
        return Ok(DeferredSourceEvidencePayload::InMemory(payload));
    }

    let spool_dir = paths.staging_dir.join("source-evidence-spool");
    fs::create_dir_all(&spool_dir).with_context(|| format!("creating {}", spool_dir.display()))?;
    let mut file = TempFileBuilder::new()
        .prefix(&spool_file_prefix(label))
        .suffix(".json")
        .tempfile_in(&spool_dir)
        .with_context(|| {
            format!("allocating deferred source-evidence in {}", spool_dir.display())
        })?;
    serde_json::to_writer(file.as_file_mut(), &payload)
        .context("serializing deferred source-evidence payload")?;
    Ok(DeferredSourceEvidencePayload::SpoolFile(file.into_temp_path()))
}

pub fn open_source_evidence_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.source_evidence_database_path)
        .with_context(|| format!("opening {}", paths.source_evidence_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        let key = key.context("database key is required for encrypted archives")?;
        apply_cipher_key(&connection, key)?;
    }
    connection.execute_batch(SOURCE_EVIDENCE_SCHEMA_SQL)?;
    Ok(connection)
}

pub(crate) fn upsert_source_batch(
    transaction: &Transaction<'_>,
    input: &SourceBatchInput,
) -> Result<i64> {
    transaction.execute(
        "INSERT INTO source_batches (
           source_profile_id, run_id, source_kind, browser_version, schema_version_text,
           schema_version_int, schema_fingerprint, parser_version, capability_snapshot_json,
           coverage_stats_json, artifact_refs_json, notes_json, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            input.source_profile_id,
            input.run_id,
            input.source_kind,
            input.browser_version,
            input.schema_version_text,
            input.schema_version_int,
            input.schema_fingerprint,
            env!("CARGO_PKG_VERSION"),
            serde_json::to_string(&input.capability_snapshot)?,
            input.coverage_stats_json,
            input.artifact_refs_json,
            input.notes_json,
            now_rfc3339(),
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

pub(crate) fn record_schema_observation(
    transaction: &Transaction<'_>,
    source_batch_id: i64,
    database_label: &str,
    observation: &SchemaObservation,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO schema_observations (source_batch_id, database_label, observation_json, recorded_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            source_batch_id,
            database_label,
            serde_json::to_string(observation)?,
            now_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Moves cold-evidence vectors out of a parsed history value once row ingest is finished.
///
/// Canonical ingest needs the full parser output while it is writing hot
/// archive rows, but source-evidence persistence only needs typed evidence and
/// native entities. Callers use this helper to drop now-unneeded URL/visit/
/// download/search-term/favicon vectors before the post-commit evidence write.
pub(crate) fn take_source_evidence_payload(parsed: &mut ParsedHistory) -> SourceEvidencePayload {
    SourceEvidencePayload {
        typed_evidence: std::mem::take(&mut parsed.typed_evidence),
        native_entities: std::mem::take(&mut parsed.native_entities),
    }
}

/// Persists typed evidence and native entities for one already-committed source batch.
///
/// The caller is responsible for writing the surrounding `source_batches` row
/// and schema observations first. This helper intentionally ignores canonical
/// visit/url rows because those already live in the hot archive.
pub(crate) fn persist_source_evidence(
    transaction: &Transaction<'_>,
    source_batch_id: i64,
    source_profile_id: i64,
    payload: &SourceEvidencePayload,
) -> Result<()> {
    let recorded_at = now_rfc3339();
    for evidence in &payload.typed_evidence.search {
        transaction.execute(
            "INSERT INTO visit_search_evidence (
               source_batch_id, source_profile_id, source_visit_id, source_url_id,
               evidence_key, evidence_value, normalized_value, source_field, recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                source_batch_id,
                source_profile_id,
                evidence.source_visit_id.map(|value| value.to_string()),
                evidence.source_url_id.map(|value| value.to_string()),
                evidence.evidence_key,
                evidence.evidence_value,
                evidence.normalized_value,
                evidence.source_field,
                recorded_at,
            ],
        )?;
    }
    for evidence in &payload.typed_evidence.navigation {
        transaction.execute(
            "INSERT INTO visit_navigation_evidence (
               source_batch_id, source_profile_id, source_visit_id, edge_kind, target_visit_id,
               target_url, transition, source_field, recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                source_batch_id,
                source_profile_id,
                evidence.source_visit_id.to_string(),
                evidence.edge_kind,
                evidence.target_visit_id.map(|value| value.to_string()),
                evidence.target_url,
                evidence.transition,
                evidence.source_field,
                recorded_at,
            ],
        )?;
    }
    for evidence in &payload.typed_evidence.engagement {
        transaction.execute(
            "INSERT INTO visit_engagement_evidence (
               source_batch_id, source_profile_id, source_visit_id, metric_key,
               metric_value_int, metric_value_real, source_field, recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                source_batch_id,
                source_profile_id,
                evidence.source_visit_id.to_string(),
                evidence.metric_key,
                evidence.metric_value_int,
                evidence.metric_value_real,
                evidence.source_field,
                recorded_at,
            ],
        )?;
    }
    for evidence in &payload.typed_evidence.context {
        transaction.execute(
            "INSERT INTO visit_context_evidence (
               source_batch_id, source_profile_id, source_visit_id, source_url_id,
               context_key, value_json, source_field, recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                source_batch_id,
                source_profile_id,
                evidence.source_visit_id.map(|value| value.to_string()),
                evidence.source_url_id.map(|value| value.to_string()),
                evidence.context_key,
                evidence.value_json,
                evidence.source_field,
                recorded_at,
            ],
        )?;
    }
    for entity in &payload.native_entities {
        transaction.execute(
            "INSERT INTO native_entities (
               source_batch_id, source_profile_id, entity_kind, native_primary_key,
               parent_native_primary_key, payload_json, metadata_json, recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                source_batch_id,
                source_profile_id,
                entity.entity_kind,
                entity.native_primary_key,
                entity.parent_native_primary_key,
                entity.payload_json,
                serde_json::to_string(&entity.metadata)?,
                recorded_at,
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn approx_source_evidence_payload_bytes(payload: &SourceEvidencePayload) -> usize {
    payload
        .typed_evidence
        .search
        .iter()
        .map(|evidence| {
            option_i64_len(evidence.source_visit_id)
                + option_i64_len(evidence.source_url_id)
                + evidence.evidence_key.len()
                + evidence.evidence_value.len()
                + option_str_len(evidence.normalized_value.as_deref())
                + evidence.source_field.len()
        })
        .sum::<usize>()
        + payload
            .typed_evidence
            .navigation
            .iter()
            .map(|evidence| {
                evidence.source_visit_id.to_string().len()
                    + evidence.edge_kind.len()
                    + option_i64_len(evidence.target_visit_id)
                    + option_str_len(evidence.target_url.as_deref())
                    + option_i64_len(evidence.transition)
                    + evidence.source_field.len()
            })
            .sum::<usize>()
        + payload
            .typed_evidence
            .engagement
            .iter()
            .map(|evidence| {
                evidence.source_visit_id.to_string().len()
                    + evidence.metric_key.len()
                    + option_i64_len(evidence.metric_value_int)
                    + option_f64_len(evidence.metric_value_real)
                    + evidence.source_field.len()
            })
            .sum::<usize>()
        + payload
            .typed_evidence
            .context
            .iter()
            .map(|evidence| {
                option_i64_len(evidence.source_visit_id)
                    + option_i64_len(evidence.source_url_id)
                    + evidence.context_key.len()
                    + evidence.value_json.len()
                    + evidence.source_field.len()
            })
            .sum::<usize>()
        + payload
            .native_entities
            .iter()
            .map(|entity| {
                entity.entity_kind.len()
                    + entity.native_primary_key.len()
                    + option_str_len(entity.parent_native_primary_key.as_deref())
                    + entity.payload_json.len()
                    + entity
                        .metadata
                        .iter()
                        .map(|(key, value)| key.len() + value.len())
                        .sum::<usize>()
            })
            .sum::<usize>()
}

fn option_i64_len(value: Option<i64>) -> usize {
    value.map(|_| std::mem::size_of::<i64>()).unwrap_or_default()
}

fn option_f64_len(value: Option<f64>) -> usize {
    value.map(|_| std::mem::size_of::<f64>()).unwrap_or_default()
}

fn option_str_len(value: Option<&str>) -> usize {
    value.map(str::len).unwrap_or_default()
}

fn spool_file_prefix(label: &str) -> String {
    let mut prefix =
        label
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { '-' }
            })
            .collect::<String>();
    prefix.truncate(24);
    if prefix.trim_matches('-').is_empty() {
        prefix = "source-evidence".to_string();
    }
    format!("pathkeep-{prefix}-")
}
