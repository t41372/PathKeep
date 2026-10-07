//! Responsibilities: page the old per-visit `visit_content_enrichments` into the
//! page-keyed table, resumably, then verify every row was carried over.
//! Not responsible for: choosing the winning row (vault-core's conversion module
//! does) or publishing search text.
//! Failure modes: a resumed run skips or repeats a page, a row vanishes, the whole
//! table is held in memory. Tests: tests/conversion.rs.
use crate::source;
use anyhow::Result;
use rusqlite::Connection;
use vault_core::{Vault, conversion::LegacyEnrichment, maintenance::Progress};
use vault_store::CancelToken;

const TABLE: &str = "visit_content_enrichments";
const PAGE: i64 = 1024;

pub(crate) fn present(source: &Connection) -> Result<bool> {
    Ok(source::tables(source)?.iter().any(|table| table == TABLE))
}

/// Converts from `after` (the last committed old row id) on; `committed` is
/// called with each new position once its page is written.
pub(crate) fn convert(
    vault: &Vault,
    source: &Connection,
    mut after: i64,
    cancel: &CancelToken,
    mut committed: impl FnMut(i64, u64) -> Result<()>,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    let total = source::count(source, TABLE)?;
    let mut processed = 0;
    loop {
        cancel.check()?;
        let rows = page(source, after)?;
        let Some(&(last, _)) = rows.last() else { break };
        processed += rows.len() as u64;
        vault_core::conversion::import_enrichments(
            vault,
            rows.into_iter().map(|(_, row)| row).collect(),
        )?;
        after = last;
        committed(after, processed)?;
        report(progress, "converting", processed, total);
    }
    let mut after = 0;
    let mut checked = 0;
    loop {
        cancel.check()?;
        let rows = page(source, after)?;
        let Some(&(last, _)) = rows.last() else { break };
        checked += rows.len() as u64;
        vault_core::conversion::verify_enrichments(
            vault,
            &rows.into_iter().map(|(_, row)| row).collect::<Vec<_>>(),
        )?;
        after = last;
        report(progress, "verifying", checked, total);
    }
    Ok(())
}

fn page(source: &Connection, after: i64) -> Result<Vec<(i64, LegacyEnrichment)>> {
    let mut statement = source.prepare(
        "SELECT id,history_id,content_source,fetch_status,fetched_at,final_url,language,readable_title,readable_text_blob_path,readable_text_bytes,text_hash,snippet_json,extraction_json,pipeline_version,extractor_version,enrichment_summary,refetch_after,http_status
         FROM visit_content_enrichments WHERE id>?1 ORDER BY id LIMIT ?2",
    )?;
    let rows = statement.query_map([after, PAGE], |r| {
        Ok((
            r.get(0)?,
            LegacyEnrichment {
                history_id: r.get(1)?,
                content_source: r.get(2)?,
                fetch_status: r.get(3)?,
                fetched_at: r.get(4)?,
                final_url: r.get(5)?,
                language: r.get(6)?,
                readable_title: r.get(7)?,
                readable_text_blob_path: r.get(8)?,
                readable_text_bytes: r.get(9)?,
                text_hash: r.get(10)?,
                snippet_json: r.get(11)?,
                extraction_json: r.get(12)?,
                pipeline_version: r.get(13)?,
                extractor_version: r.get(14)?,
                enrichment_summary: r.get(15)?,
                refetch_after: r.get(16)?,
                http_status: r.get(17)?,
            },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn report(progress: &mut impl FnMut(Progress), phase: &str, processed: u64, total: u64) {
    progress(Progress {
        phase: phase.into(),
        database: Some("Intelligence".into()),
        table: Some(TABLE.into()),
        processed,
        total,
    });
}
