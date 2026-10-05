//! Responsibilities: preserve parser provenance and typed/native evidence in bounded writes.
//! Not responsible for: canonical visits, index consumption or file staging.
//! Failure modes: optional fields disappear, evidence is lost before a cursor
//! commits, large evidence exceeds the batch bound, keys are ignored.
//! Tests: source_evidence_tests and ingest::core_tests.
use crate::utils::now_rfc3339;
use anyhow::Result;
use browser_history_parser::{
    CapabilitySnapshot, NativeEntity, SchemaObservation, TypedEvidenceBatch,
};
use serde::{Deserialize, Serialize};
use vault_store::{Db, Priority, Session, params};
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

pub(crate) fn upsert_source_batch(
    transaction: &Session<'_>,
    input: &SourceBatchInput,
) -> Result<i64> {
    let id = transaction.query_one(
        "INSERT INTO source_batches (
           source_profile_id, run_id, source_kind, browser_version, schema_version_text,
           schema_version_int, schema_fingerprint, parser_version, capability_snapshot_json,
           coverage_stats_json, artifact_refs_json, notes_json, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) RETURNING id",
        params![
            input.source_profile_id,
            input.run_id,
            input.source_kind.clone(),
            input.browser_version.clone(),
            input.schema_version_text.clone(),
            input.schema_version_int,
            input.schema_fingerprint.clone(),
            env!("CARGO_PKG_VERSION"),
            serde_json::to_string(&input.capability_snapshot)?,
            input.coverage_stats_json.clone(),
            input.artifact_refs_json.clone(),
            input.notes_json.clone(),
            now_rfc3339(),
        ],
        |r| Ok(r.get::<i64>(0)?),
    )?;
    Ok(id)
}

pub(crate) fn record_schema_observation(
    transaction: &Session<'_>,
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

/// Persists typed evidence and native entities for one already-committed source batch.
///
/// The caller is responsible for writing the surrounding `source_batches` row
/// and schema observations first. This helper intentionally ignores canonical
/// visit/url rows because those already live in the hot archive.
pub(crate) fn persist_source_evidence(
    transaction: &Session<'_>,
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
                evidence.evidence_key.clone(),
                evidence.evidence_value.clone(),
                evidence.normalized_value.clone(),
                evidence.source_field.clone(),
                recorded_at.clone(),
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
                evidence.edge_kind.clone(),
                evidence.target_visit_id.map(|value| value.to_string()),
                evidence.target_url.clone(),
                evidence.transition,
                evidence.source_field.clone(),
                recorded_at.clone(),
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
                evidence.metric_key.clone(),
                evidence.metric_value_int,
                evidence.metric_value_real,
                evidence.source_field.clone(),
                recorded_at.clone(),
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
                evidence.context_key.clone(),
                evidence.value_json.clone(),
                evidence.source_field.clone(),
                recorded_at.clone(),
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
                entity.entity_kind.clone(),
                entity.native_primary_key.clone(),
                entity.parent_native_primary_key.clone(),
                entity.payload_json.clone(),
                serde_json::to_string(&entity.metadata)?,
                recorded_at.clone(),
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_evidence_tests.rs"]
mod tests;
/// Bounded cold writes complete before a canonical batch advances its watermark.
pub(crate) struct EvidenceWriter<'a> {
    vault: &'a crate::Vault,
    batch_id: i64,
    profile_id: i64,
    counts: EvidenceCounts,
}

impl<'a> EvidenceWriter<'a> {
    pub(crate) fn new(vault: &'a crate::Vault, batch_id: i64, profile_id: i64) -> Self {
        Self { vault, batch_id, profile_id, counts: EvidenceCounts::default() }
    }
    pub(crate) fn counts(&self) -> EvidenceCounts {
        self.counts.clone()
    }
    pub(crate) fn batch_id(&self) -> i64 {
        self.batch_id
    }
    pub(crate) fn push(&mut self, payload: SourceEvidencePayload) -> Result<()> {
        const ROWS: usize = 10_000;
        self.counts.add(&payload);
        // Each family uses the same row bound; no whole-import evidence vector survives.
        for rows in payload.typed_evidence.search.chunks(ROWS) {
            let mut part = SourceEvidencePayload::default();
            part.typed_evidence.search = rows.to_vec();
            self.write(part)?;
        }
        for rows in payload.typed_evidence.navigation.chunks(ROWS) {
            let mut part = SourceEvidencePayload::default();
            part.typed_evidence.navigation = rows.to_vec();
            self.write(part)?;
        }
        for rows in payload.typed_evidence.engagement.chunks(ROWS) {
            let mut part = SourceEvidencePayload::default();
            part.typed_evidence.engagement = rows.to_vec();
            self.write(part)?;
        }
        for rows in payload.typed_evidence.context.chunks(ROWS) {
            let mut part = SourceEvidencePayload::default();
            part.typed_evidence.context = rows.to_vec();
            self.write(part)?;
        }
        for rows in payload.native_entities.chunks(ROWS) {
            self.write(SourceEvidencePayload {
                native_entities: rows.to_vec(),
                ..SourceEvidencePayload::default()
            })?;
        }
        Ok(())
    }
    fn write(&self, payload: SourceEvidencePayload) -> Result<()> {
        let (batch, profile) = (self.batch_id, self.profile_id);
        self.vault.write(Db::SourceEvidence, Priority::Batch, move |s| {
            persist_source_evidence(s, batch, profile, &payload)
        })
    }
}

/// Coverage is accumulated with committed evidence, never from an unbounded report.
#[derive(Debug, Clone, Default)]
pub(crate) struct EvidenceCounts {
    pub search: usize,
    pub navigation: usize,
    pub engagement: usize,
    pub context: usize,
    pub native: usize,
}
impl EvidenceCounts {
    fn add(&mut self, payload: &SourceEvidencePayload) {
        self.search += payload.typed_evidence.search.len();
        self.navigation += payload.typed_evidence.navigation.len();
        self.engagement += payload.typed_evidence.engagement.len();
        self.context += payload.typed_evidence.context.len();
        self.native += payload.native_entities.len();
    }
    pub(crate) fn coverage(
        &self,
        urls: usize,
        visits: usize,
        downloads: usize,
        terms: usize,
    ) -> String {
        serde_json::json!({"urls":urls,"visits":visits,"downloads":downloads,"searchTerms":terms,
            "searchEvidence":self.search,"navigationEvidence":self.navigation,"engagementEvidence":self.engagement,"contextEvidence":self.context,"nativeEntities":self.native}).to_string()
    }
}
