//! Responsibilities: stream Takeout payloads into bounded archive/evidence writes.
//! Not responsible for: file discovery, batch review or run finalization.
//! Failure modes: source renames duplicate visits, missing URLs create orphan visits,
//! evidence loss precedes a committed cursor, imports retain whole-archive maps.
//! Tests: ingest::dedup_scenarios_takeout and takeout::tests.

use super::*;
use crate::{
    Vault,
    archive::ingest::{
        ArchiveChunkConsumer, ArchiveIngestProgress, BATCH_ROWS, ImportOrigin, WriteContext,
        parser::Watermark,
    },
};
use browser_history_parser::{
    HistoryBatchConsumer, SourceEvidenceChunk, StreamHistoryError,
    takeout::{
        TakeoutSourceEvidenceChunk, TakeoutSourceEvidenceConsumer, TakeoutStreamOptions,
        stream_payload_reader_with_sink,
    },
};
use vault_store::{CancelToken, Db, Priority, Session, params};

pub(super) type TakeoutPayloadProgress = ArchiveIngestProgress;
pub(super) struct TakeoutPayloadImportContext<'a> {
    pub vault: &'a Vault,
    pub run_id: i64,
    pub batch_id: i64,
    pub source_profile_id: i64,
    /// `takeout_profile` of the payload's kind.
    pub profile_key: &'static str,
    pub cancel: &'a CancelToken,
}

impl TakeoutSourceEvidenceConsumer<anyhow::Error> for ArchiveChunkConsumer<'_> {
    fn source_evidence(&mut self, chunk: TakeoutSourceEvidenceChunk) -> Result<()> {
        HistoryBatchConsumer::source_evidence(
            self,
            SourceEvidenceChunk {
                typed_evidence: chunk.typed_evidence,
                native_entities: chunk.native_entities,
            },
        )
    }
}

pub(super) fn import_supported_payload_with_progress<'a>(
    context: TakeoutPayloadImportContext<'a>,
    file: ClassifiedTakeoutFile<'a>,
    kind: &str,
    reader: &mut dyn std::io::Read,
    progress: Option<Box<dyn FnMut(TakeoutPayloadProgress) + 'a>>,
) -> Result<ImportedPayload> {
    let TakeoutPayloadImportContext {
        vault,
        run_id,
        batch_id,
        source_profile_id,
        profile_key,
        cancel,
    } = context;
    let path = file.file.path.clone();
    let source_batch = SourceBatchInput {
        source_profile_id,
        run_id: Some(run_id),
        source_kind: "takeout".into(),
        browser_version: None,
        schema_version_text: Some(kind.into()),
        schema_version_int: None,
        schema_fingerprint: String::new(),
        capability_snapshot: browser_history_parser::CapabilitySnapshot::default(),
        coverage_stats_json: "{}".into(),
        artifact_refs_json: Some(json!({"sourcePath":path,"payloadKind":kind}).to_string()),
        notes_json: None,
    };
    let source_batch_id = vault.write(Db::SourceEvidence, Priority::Batch, move |s| {
        upsert_source_batch(s, &source_batch)
    })?;
    let write = WriteContext {
        run_id,
        source_profile_id,
        profile_id: profile_key.into(),
        browser_family: "takeout".into(),
        import: Some(ImportOrigin {
            batch_id,
            source_kind: "takeout".into(),
            source_path: Some(path),
        }),
    };
    let mut consumer = ArchiveChunkConsumer::new(
        vault,
        write,
        Watermark::default(),
        cancel,
        progress,
        source_batch_id,
    );
    let report = stream_payload_reader_with_sink(
        &file.file.path,
        kind,
        reader,
        BATCH_ROWS,
        TakeoutStreamOptions {
            collect_source_evidence: true,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .map_err(takeout_stream_error_to_anyhow)
    .with_context(|| format!("parsing Takeout payload {} ({kind})", file.file.path))?;
    let counts = consumer.finish()?;
    let skipped = report.skipped_missing_visit_time + counts.skipped_visits;
    let result = ImportedPayload {
        stats: ImportStats {
            imported_items: counts.new_visits,
            duplicate_items: counts.visit_count - counts.new_visits - counts.same_visits,
            same_visit_items: counts.same_visits,
            replaced_profiles: counts.replaced_profiles.clone(),
            skipped_items: skipped,
        },
        record_count: report.record_count,
        recognized_file: file_report_from_match(
            file,
            if skipped > 0 { "previewed-with-skips" } else { "previewed" },
            report.record_count,
            None,
        ),
        earliest_visit_iso: report.earliest_visit_iso.clone(),
        latest_visit_iso: report.latest_visit_iso.clone(),
    };
    vault.write(Db::SourceEvidence,Priority::Batch,move |s| {
        record_schema_observation(s,source_batch_id,"takeout-payload",&report.history.schema_observation)?;
        s.execute("UPDATE source_batches SET schema_fingerprint=?2 WHERE id=?1",params![source_batch_id,sha256_hex(&serde_json::to_vec(&report.history.schema_observation)?)])?;
        s.execute("UPDATE source_batches SET capability_snapshot_json=?2,coverage_stats_json=?3,notes_json=?4 WHERE id=?1",params![
            source_batch_id,serde_json::to_string(&report.history.capability_snapshot)?,
            counts.evidence_counts.coverage(counts.url_count,counts.visit_count,counts.download_count,counts.search_term_count),serde_json::to_string(&report.history.warnings)?])?;
        Ok(())
    })?;
    Ok(result)
}

fn takeout_stream_error_to_anyhow(error: StreamHistoryError<anyhow::Error>) -> anyhow::Error {
    match error {
        StreamHistoryError::Parse(e) => e.into(),
        StreamHistoryError::Consumer(e) => e,
    }
}

pub(super) fn upsert_takeout_profile(
    archive: &Session<'_>,
    profile_id: &str,
    source: &Path,
) -> Result<i64> {
    let enabled = archive
        .query("SELECT enabled FROM source_profiles WHERE profile_key=?1", [profile_id], |r| {
            Ok(r.get::<i64>(0)?)
        })?
        .pop();
    if enabled != Some(1) {
        crate::archive::totals::add(archive, "sourceProfiles", 1)?;
    }
    archive.execute(
        "INSERT INTO source_profiles (
           browser_kind,
           browser_family,
           browser_product,
           browser_version,
           profile_name,
           profile_path,
           discovered_at,
           enabled,
           profile_key,
           user_name,
           updated_at
         )
         VALUES ('takeout', 'takeout', 'takeout', 'takeout', ?1, ?2, ?3, 1, ?4, NULL, ?3)
         ON CONFLICT(profile_key) DO UPDATE SET
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           browser_family = excluded.browser_family,
           browser_product = excluded.browser_product,
           browser_version = excluded.browser_version,
           updated_at = excluded.updated_at,
           enabled = 1",
        params![
            if profile_id == takeout_profile(KIND_MY_ACTIVITY_JSON) {
                "Google My Activity (Chrome)"
            } else {
                "Imported browser history"
            },
            source.display().to_string(),
            now_rfc3339(),
            profile_id,
        ],
    )?;
    archive
        .query_one("SELECT id FROM source_profiles WHERE profile_key = ?1", [profile_id], |row| {
            Ok(row.get(0)?)
        })
        .map_err(Into::into)
}
