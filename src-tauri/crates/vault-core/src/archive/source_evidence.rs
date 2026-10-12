//! Responsibilities: preserve parser provenance and typed/native evidence in bounded writes.
//! Not responsible for: canonical visits, index consumption or file staging.
//! Failure modes: optional fields disappear, evidence is lost before a cursor
//! commits, large evidence exceeds the batch bound, keys are ignored, evidence the
//! archive already holds is written again or a changed value is taken for a copy.
//! Tests: source_evidence_tests and ingest::core_tests.
use crate::utils::now_rfc3339;
use anyhow::Result;
use browser_history_parser::{
    CapabilitySnapshot, NativeEntity, SchemaObservation, TypedEvidenceBatch,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vault_store::{Db, Priority, Session, Value, params};
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

/// One evidence table and the columns that say what a row is. Two rows of the same source profile
/// with the same values in these are the same evidence read again (a file imported twice, an
/// import carried on, a URL a backup scans again); the batch and the time it was written are not
/// part of it. A value that changed (a visit's duration, written when its tab closed) is new
/// evidence and gets a row of its own.
pub struct EvidenceFamily {
    pub table: &'static str,
    pub content: &'static [&'static str],
}

/// Every evidence table, for the writer below and for de-duplicating an old-format archive
/// (`conversion::dedupe_source_evidence`), which must key rows the same way.
pub const FAMILIES: [EvidenceFamily; 5] = [
    EvidenceFamily {
        table: "visit_search_evidence",
        content: &[
            "source_visit_id",
            "source_url_id",
            "evidence_key",
            "evidence_value",
            "normalized_value",
            "source_field",
        ],
    },
    EvidenceFamily {
        table: "visit_navigation_evidence",
        content: &[
            "source_visit_id",
            "edge_kind",
            "target_visit_id",
            "target_url",
            "transition",
            "source_field",
        ],
    },
    EvidenceFamily {
        table: "visit_engagement_evidence",
        content: &[
            "source_visit_id",
            "metric_key",
            "metric_value_int",
            "metric_value_real",
            "source_field",
        ],
    },
    EvidenceFamily {
        table: "visit_context_evidence",
        content: &["source_visit_id", "source_url_id", "context_key", "value_json", "source_field"],
    },
    // The metadata says where a record was read (a Takeout file's path), not what it holds: the
    // same export read from another folder is the same evidence, and keeps its first metadata.
    EvidenceFamily {
        table: "native_entities",
        content: &[
            "entity_kind",
            "native_primary_key",
            "parent_native_primary_key",
            "payload_json",
        ],
    },
];

const SEARCH: usize = 0;
const NAVIGATION: usize = 1;
const ENGAGEMENT: usize = 2;
const CONTEXT: usize = 3;
const NATIVE: usize = 4;

/// The `content_key` of a row: its family, source profile and content values, hashed. The values
/// are those stored, so a key computed from a row read back matches the writer's.
pub fn content_key(table: &str, source_profile_id: i64, content: &[Value]) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(table.as_bytes());
    hash.update([0]);
    hash.update(source_profile_id.to_le_bytes());
    for value in content {
        match value {
            Value::Null => hash.update([0]),
            Value::Integer(value) => {
                hash.update([1]);
                hash.update(value.to_le_bytes());
            }
            Value::Real(value) => {
                hash.update([2]);
                hash.update(value.to_bits().to_le_bytes());
            }
            Value::Text(value) => {
                hash.update([3]);
                hash.update((value.len() as u64).to_le_bytes());
                hash.update(value.as_bytes());
            }
            Value::Blob(value) => {
                hash.update([4]);
                hash.update((value.len() as u64).to_le_bytes());
                hash.update(value);
            }
        }
    }
    // 128 bits: no two of the archive's evidence rows collide by chance.
    hash.finalize()[..16].to_vec()
}

fn text(value: impl ToString) -> Value {
    Value::Text(value.to_string())
}

fn optional<T>(value: Option<T>, into: impl FnOnce(T) -> Value) -> Value {
    value.map_or(Value::Null, into)
}

/// Writes one family's rows, skipping the evidence a row already holds. `rows` are each row's
/// content values and then its other columns (`extra`), in order. Returns the rows written.
fn insert_family(
    transaction: &Session<'_>,
    family: &EvidenceFamily,
    source_batch_id: i64,
    source_profile_id: i64,
    recorded_at: &str,
    extra: &[&str],
    rows: Vec<Vec<Value>>,
) -> Result<u64> {
    if rows.is_empty() {
        return Ok(0);
    }
    let columns = ["source_batch_id", "source_profile_id"]
        .iter()
        .chain(family.content)
        .chain(extra)
        .chain(&["recorded_at", "content_key"])
        .copied()
        .collect::<Vec<_>>();
    let sql = format!(
        "INSERT INTO {}({}) VALUES ({}) ON CONFLICT(content_key) DO NOTHING",
        family.table,
        columns.join(","),
        (1..=columns.len()).map(|n| format!("?{n}")).collect::<Vec<_>>().join(","),
    );
    let rows = rows
        .into_iter()
        .map(|values| {
            let key = content_key(family.table, source_profile_id, &values[..family.content.len()]);
            let mut row = vec![Value::Integer(source_batch_id), Value::Integer(source_profile_id)];
            row.extend(values);
            row.push(text(recorded_at));
            row.push(Value::Blob(key));
            row
        })
        .collect();
    Ok(transaction.execute_rows(&sql, rows)?)
}

/// Persists typed evidence and native entities for one already-committed source batch, each piece
/// once: what the archive already holds for the profile is not written again. Returns the rows
/// written.
///
/// The caller is responsible for writing the surrounding `source_batches` row
/// and schema observations first. This helper intentionally ignores canonical
/// visit/url rows because those already live in the hot archive.
pub(crate) fn persist_source_evidence(
    transaction: &Session<'_>,
    source_batch_id: i64,
    source_profile_id: i64,
    payload: &SourceEvidencePayload,
) -> Result<u64> {
    let recorded_at = now_rfc3339();
    let evidence = &payload.typed_evidence;
    let write = |family: usize, extra: &[&str], rows: Vec<Vec<Value>>| {
        insert_family(
            transaction,
            &FAMILIES[family],
            source_batch_id,
            source_profile_id,
            &recorded_at,
            extra,
            rows,
        )
    };
    let mut written = write(
        SEARCH,
        &[],
        evidence
            .search
            .iter()
            .map(|e| {
                vec![
                    optional(e.source_visit_id, text),
                    optional(e.source_url_id, text),
                    text(&e.evidence_key),
                    text(&e.evidence_value),
                    optional(e.normalized_value.as_ref(), text),
                    text(&e.source_field),
                ]
            })
            .collect(),
    )?;
    written += write(
        NAVIGATION,
        &[],
        evidence
            .navigation
            .iter()
            .map(|e| {
                vec![
                    text(e.source_visit_id),
                    text(&e.edge_kind),
                    optional(e.target_visit_id, text),
                    optional(e.target_url.as_ref(), text),
                    optional(e.transition, Value::Integer),
                    text(&e.source_field),
                ]
            })
            .collect(),
    )?;
    written += write(
        ENGAGEMENT,
        &[],
        evidence
            .engagement
            .iter()
            .map(|e| {
                vec![
                    text(e.source_visit_id),
                    text(&e.metric_key),
                    optional(e.metric_value_int, Value::Integer),
                    optional(e.metric_value_real, Value::Real),
                    text(&e.source_field),
                ]
            })
            .collect(),
    )?;
    written += write(
        CONTEXT,
        &[],
        evidence
            .context
            .iter()
            .map(|e| {
                vec![
                    optional(e.source_visit_id, text),
                    optional(e.source_url_id, text),
                    text(&e.context_key),
                    text(&e.value_json),
                    text(&e.source_field),
                ]
            })
            .collect(),
    )?;
    written += write(
        NATIVE,
        &["metadata_json"],
        payload
            .native_entities
            .iter()
            .map(|entity| {
                Ok(vec![
                    text(&entity.entity_kind),
                    text(&entity.native_primary_key),
                    optional(entity.parent_native_primary_key.as_ref(), text),
                    text(&entity.payload_json),
                    text(serde_json::to_string(&entity.metadata)?),
                ])
            })
            .collect::<Result<_>>()?,
    )?;
    Ok(written)
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
    fn write(&mut self, payload: SourceEvidencePayload) -> Result<()> {
        let (batch, profile) = (self.batch_id, self.profile_id);
        self.counts.written += self.vault.write(Db::SourceEvidence, Priority::Batch, move |s| {
            persist_source_evidence(s, batch, profile, &payload)
        })?;
        Ok(())
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
    /// Rows written: the rows above less the evidence the archive already held.
    pub written: u64,
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
            "searchEvidence":self.search,"navigationEvidence":self.navigation,"engagementEvidence":self.engagement,"contextEvidence":self.context,"nativeEntities":self.native,"evidenceRowsWritten":self.written}).to_string()
    }
}
