//! Incremental builders for deferred cold source-evidence payloads.
//!
//! ## Responsibilities
//! - Accumulate source-evidence chunks while canonical import is still running.
//! - Spill oversized evidence batches into `staging/source-evidence-spool/`
//!   before one payload grows into an unbounded in-memory buffer.
//! - Return the same deferred payload contract used by post-commit cold
//!   archive persistence.
//!
//! ## Not responsible for
//! - Writing source-evidence rows into SQLite.
//! - Owning canonical archive counters or import-batch review artifacts.
//! - Defining parser-side source-evidence extraction rules.
//!
//! ## Dependencies
//! - `super::source_evidence` for deferred payload types and byte accounting.
//! - `tempfile` for chunk-spool files that self-delete once import plans drop.
//!
//! ## Performance notes
//! - This builder keeps at most one threshold-sized chunk in memory before it
//!   spills to disk, which bounds the Takeout import hot path by chunk size
//!   rather than by the full payload's native evidence volume.

use super::source_evidence::{
    DeferredSourceEvidencePayload, SOURCE_EVIDENCE_SPOOL_THRESHOLD_BYTES, SourceEvidencePayload,
    approx_source_evidence_payload_bytes,
};
use crate::config::ProjectPaths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, io::Write};
use tempfile::{Builder as TempFileBuilder, NamedTempFile};

/// Counted summary of a cold source-evidence payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SourceEvidenceCounts {
    pub search_evidence: usize,
    pub navigation_evidence: usize,
    pub engagement_evidence: usize,
    pub context_evidence: usize,
    pub native_entities: usize,
}

impl SourceEvidenceCounts {
    /// Builds counters from one payload.
    pub(crate) fn from_payload(payload: &SourceEvidencePayload) -> Self {
        Self {
            search_evidence: payload.typed_evidence.search.len(),
            navigation_evidence: payload.typed_evidence.navigation.len(),
            engagement_evidence: payload.typed_evidence.engagement.len(),
            context_evidence: payload.typed_evidence.context.len(),
            native_entities: payload.native_entities.len(),
        }
    }

    /// Adds one payload worth of counts into the running total.
    pub(crate) fn add_payload(&mut self, payload: &SourceEvidencePayload) {
        let next = Self::from_payload(payload);
        self.search_evidence += next.search_evidence;
        self.navigation_evidence += next.navigation_evidence;
        self.engagement_evidence += next.engagement_evidence;
        self.context_evidence += next.context_evidence;
        self.native_entities += next.native_entities;
    }
}

/// Builds coverage stats JSON from canonical row counts plus the deferred cold payload.
pub(crate) fn coverage_stats_json_from_parts(
    urls: usize,
    visits: usize,
    downloads: usize,
    search_terms: usize,
    payload: &SourceEvidencePayload,
) -> String {
    coverage_stats_json_from_counts(
        urls,
        visits,
        downloads,
        search_terms,
        &SourceEvidenceCounts::from_payload(payload),
    )
}

/// Builds coverage stats JSON from canonical row counts plus a counted cold payload summary.
pub(crate) fn coverage_stats_json_from_counts(
    urls: usize,
    visits: usize,
    downloads: usize,
    search_terms: usize,
    counts: &SourceEvidenceCounts,
) -> String {
    json!({
        "urls": urls,
        "visits": visits,
        "downloads": downloads,
        "searchTerms": search_terms,
        "searchEvidence": counts.search_evidence,
        "navigationEvidence": counts.navigation_evidence,
        "engagementEvidence": counts.engagement_evidence,
        "contextEvidence": counts.context_evidence,
        "nativeEntities": counts.native_entities,
    })
    .to_string()
}

/// Incrementally builds one deferred source-evidence payload.
pub(crate) struct DeferredSourceEvidenceBuilder {
    paths: ProjectPaths,
    label: String,
    counts: SourceEvidenceCounts,
    pending_payload: SourceEvidencePayload,
    pending_bytes: usize,
    spool_file: Option<NamedTempFile>,
}

impl DeferredSourceEvidenceBuilder {
    /// Starts a new bounded-memory builder for one source payload.
    pub(crate) fn new(paths: &ProjectPaths, label: &str) -> Self {
        Self {
            paths: paths.clone(),
            label: label.to_string(),
            counts: SourceEvidenceCounts::default(),
            pending_payload: SourceEvidencePayload::default(),
            pending_bytes: 0,
            spool_file: None,
        }
    }

    /// Adds one source-evidence chunk to the deferred payload.
    pub(crate) fn push(&mut self, payload: SourceEvidencePayload) -> Result<()> {
        if payload_is_empty(&payload) {
            return Ok(());
        }

        self.counts.add_payload(&payload);
        let payload_bytes = approx_source_evidence_payload_bytes(&payload);
        if let Some(file) = self.spool_file.as_mut() {
            append_payload_chunk(file, &payload)?;
            return Ok(());
        }

        if self.pending_bytes + payload_bytes <= SOURCE_EVIDENCE_SPOOL_THRESHOLD_BYTES {
            merge_payload(&mut self.pending_payload, payload);
            self.pending_bytes += payload_bytes;
            return Ok(());
        }

        let mut file = allocate_spool_file(&self.paths, &self.label)?;
        if !payload_is_empty(&self.pending_payload) {
            append_payload_chunk(&mut file, &self.pending_payload)?;
            self.pending_payload = SourceEvidencePayload::default();
            self.pending_bytes = 0;
        }
        append_payload_chunk(&mut file, &payload)?;
        self.spool_file = Some(file);
        Ok(())
    }

    /// Returns the counted summary accumulated so far.
    pub(crate) fn counts(&self) -> SourceEvidenceCounts {
        self.counts
    }

    /// Finalizes the deferred payload for post-commit cold archive persistence.
    pub(crate) fn finish(mut self) -> Result<DeferredSourceEvidencePayload> {
        if let Some(mut file) = self.spool_file.take() {
            if !payload_is_empty(&self.pending_payload) {
                append_payload_chunk(&mut file, &self.pending_payload)?;
            }
            return Ok(DeferredSourceEvidencePayload::ChunkFile(file.into_temp_path()));
        }

        Ok(DeferredSourceEvidencePayload::InMemory(self.pending_payload))
    }
}

fn payload_is_empty(payload: &SourceEvidencePayload) -> bool {
    payload.typed_evidence.search.is_empty()
        && payload.typed_evidence.navigation.is_empty()
        && payload.typed_evidence.engagement.is_empty()
        && payload.typed_evidence.context.is_empty()
        && payload.native_entities.is_empty()
}

fn merge_payload(target: &mut SourceEvidencePayload, mut payload: SourceEvidencePayload) {
    target.typed_evidence.search.append(&mut payload.typed_evidence.search);
    target.typed_evidence.navigation.append(&mut payload.typed_evidence.navigation);
    target.typed_evidence.engagement.append(&mut payload.typed_evidence.engagement);
    target.typed_evidence.context.append(&mut payload.typed_evidence.context);
    target.native_entities.append(&mut payload.native_entities);
}

fn allocate_spool_file(paths: &ProjectPaths, label: &str) -> Result<NamedTempFile> {
    let spool_dir = paths.staging_dir.join("source-evidence-spool");
    fs::create_dir_all(&spool_dir).with_context(|| format!("creating {}", spool_dir.display()))?;
    TempFileBuilder::new()
        .prefix(&spool_file_prefix(label))
        .suffix(".jsonl")
        .tempfile_in(&spool_dir)
        .with_context(|| format!("allocating deferred source-evidence in {}", spool_dir.display()))
}

fn append_payload_chunk(file: &mut NamedTempFile, payload: &SourceEvidencePayload) -> Result<()> {
    serde_json::to_writer(file.as_file_mut(), payload)
        .context("serializing deferred source-evidence chunk")?;
    file.as_file_mut().write_all(b"\n").context("terminating deferred source-evidence chunk")?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use browser_history_parser::NativeEntity;
    use std::collections::BTreeMap;

    fn native_payload(label: &str, payload_bytes: usize) -> SourceEvidencePayload {
        SourceEvidencePayload {
            typed_evidence: Default::default(),
            native_entities: vec![NativeEntity {
                entity_kind: "history".to_string(),
                native_primary_key: label.to_string(),
                parent_native_primary_key: None,
                payload_json: "x".repeat(payload_bytes),
                metadata: BTreeMap::new(),
            }],
        }
    }

    #[test]
    fn deferred_builder_skips_empty_payloads_and_spools_large_chunks() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(root.path());
        let mut builder = DeferredSourceEvidenceBuilder::new(&paths, "Chrome Default");

        builder.push(SourceEvidencePayload::default()).expect("empty payload");
        assert_eq!(builder.counts(), SourceEvidenceCounts::default());

        builder.push(native_payload("small", 64)).expect("small payload");
        assert_eq!(builder.counts().native_entities, 1);

        builder
            .push(native_payload("large", SOURCE_EVIDENCE_SPOOL_THRESHOLD_BYTES + 1))
            .expect("large payload");
        builder.push(native_payload("next", 64)).expect("append spooled payload");
        assert_eq!(builder.counts().native_entities, 3);

        match builder.finish().expect("finish builder") {
            DeferredSourceEvidencePayload::ChunkFile(path) => {
                let content = fs::read_to_string(&path).expect("chunk file");
                assert_eq!(content.lines().count(), 3);
            }
            DeferredSourceEvidencePayload::InMemory(_)
            | DeferredSourceEvidencePayload::SpoolFile(_) => {
                panic!("large payload should finish as a chunk file")
            }
        }
    }

    #[test]
    fn deferred_builder_flushes_pending_payload_when_spool_is_already_allocated() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(root.path());
        let mut builder = DeferredSourceEvidenceBuilder::new(&paths, "manual invariant");
        builder.spool_file = Some(allocate_spool_file(&paths, "manual invariant").expect("spool"));
        builder.pending_payload = native_payload("pending", 64);

        match builder.finish().expect("finish builder") {
            DeferredSourceEvidencePayload::ChunkFile(path) => {
                let content = fs::read_to_string(&path).expect("chunk file");
                assert_eq!(content.lines().count(), 1);
            }
            DeferredSourceEvidencePayload::InMemory(_)
            | DeferredSourceEvidencePayload::SpoolFile(_) => {
                panic!("allocated spool should finish as a chunk file")
            }
        }
    }

    #[test]
    fn spool_file_prefix_falls_back_for_symbol_only_labels() {
        assert_eq!(spool_file_prefix("###"), "pathkeep-source-evidence-");
    }
}
