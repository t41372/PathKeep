//! Responsibilities: submit bounded parser batches to the archive writer queue.
//! Not responsible for: parsing, run completion, manifests or search indexing.
//! Failure modes: archive-sized maps, cursor/progress ahead of committed rows,
//! URL cursor skips dependent terms, imports advance live cursors, duplicates widen bounds, missing URL parents, stale documents, cancellation
//! discards committed batches, interactive jobs wait for the whole import.
//! Tests: core_tests, concurrency_tests and dedup_scenarios*.

use super::{
    parser::{Watermark, save_watermark},
    selection::*,
    writes::*,
};
use crate::{
    Vault,
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::Result;
use browser_history_parser::{
    HistoryBatchConsumer, ParsedDownload, ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit,
    SourceEvidenceChunk,
};
use std::collections::{BTreeSet, HashMap};
use vault_store::{CancelToken, Db, Priority, Session, params};

pub(crate) const BATCH_ROWS: usize = 10_000;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ArchiveIngestProgress {
    pub processed_records: usize,
    pub imported_records: usize,
    pub duplicate_records: usize,
    pub skipped_records: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ArchiveStreamProgress {
    pub evidence_counts: crate::archive::source_evidence::EvidenceCounts,
    pub first_visit_iso: Option<String>,
    pub last_visit_iso: Option<String>,
    pub new_urls: usize,
    pub new_visits: usize,
    pub new_downloads: usize,
    pub skipped_visits: usize,
    pub inserted_search_terms: usize,
    pub url_count: usize,
    pub visit_count: usize,
    pub download_count: usize,
    pub search_term_count: usize,
    pub last_visit_id: i64,
    pub last_url_marker: Option<i64>,
    pub last_download_id: Option<i64>,
    pub last_favicon_marker: Option<i64>,
}

impl ArchiveStreamProgress {
    fn event(&self) -> ArchiveIngestProgress {
        ArchiveIngestProgress {
            processed_records: self.visit_count + self.skipped_visits,
            imported_records: self.new_visits,
            duplicate_records: self.visit_count - self.new_visits,
            skipped_records: self.skipped_visits,
        }
    }
}

#[derive(Clone)]
pub(crate) struct WriteContext {
    pub run_id: i64,
    pub source_profile_id: i64,
    pub profile_id: String,
    pub browser_family: String,
    pub import: Option<ImportOrigin>,
}

impl WriteContext {
    fn tracks_watermark(&self) -> bool {
        self.import.as_ref().is_none_or(|origin| origin.source_kind == "takeout")
    }
}

#[derive(Clone)]
pub(crate) struct ImportOrigin {
    pub batch_id: i64,
    pub source_kind: String,
    pub source_path: Option<String>,
}

enum Batch {
    Urls(Vec<(ParsedUrl, String)>),
    Visits(Vec<(ParsedVisit, String)>),
    Downloads(Vec<(ParsedDownload, String)>),
    Terms(Vec<ParsedSearchTerm>),
    Favicons(Vec<(ParsedFavicon, String)>),
}

/// The producer retains one bounded batch and waits for each committed reply.
pub(crate) struct ArchiveChunkConsumer<'a> {
    vault: &'a Vault,
    context: WriteContext,
    watermark: Watermark,
    progress: ArchiveStreamProgress,
    cancel: &'a CancelToken,
    report_progress: Option<Box<dyn FnMut(ArchiveIngestProgress) + 'a>>,
    evidence: crate::archive::source_evidence::EvidenceWriter<'a>,
    pending: Vec<Batch>,
}

impl<'a> ArchiveChunkConsumer<'a> {
    pub(crate) fn new(
        vault: &'a Vault,
        context: WriteContext,
        watermark: Watermark,
        cancel: &'a CancelToken,
        report_progress: Option<Box<dyn FnMut(ArchiveIngestProgress) + 'a>>,
        source_batch_id: i64,
    ) -> Self {
        let evidence = crate::archive::source_evidence::EvidenceWriter::new(
            vault,
            source_batch_id,
            context.source_profile_id,
        );
        Self {
            vault,
            context,
            watermark,
            progress: ArchiveStreamProgress::default(),
            cancel,
            report_progress,
            evidence,
            pending: Vec::new(),
        }
    }

    pub(crate) fn finish(mut self) -> Result<ArchiveStreamProgress> {
        self.flush()?;
        if self.context.tracks_watermark() {
            let context = self.context.clone();
            let mut watermark = self.watermark.clone();
            watermark.url_scan = None;
            self.vault.write(Db::Archive, Priority::Batch, move |s| {
                save_watermark(s, &context.profile_id, &watermark)?;
                Ok(())
            })?;
        }
        self.progress.evidence_counts = self.evidence.counts();
        Ok(self.progress)
    }

    fn push(&mut self, batch: Batch) -> Result<()> {
        self.cancel.check()?;
        // Takeout emits URLs, visits, then their evidence; browser parsers emit
        // one canonical batch followed by evidence. Neither retains a whole file.
        if self.pending.len() == 2 {
            self.flush()?;
        }
        self.pending.push(batch);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        for batch in std::mem::take(&mut self.pending) {
            self.cancel.check()?;
            let context = self.context.clone();
            let mut progress = self.progress.clone();
            let mut watermark = self.watermark.clone();
            watermark.last_source_batch_id = Some(self.evidence.batch_id());
            let report_visits = matches!(batch, Batch::Visits(_));
            let (next, cursor) = self.vault.write(Db::Archive, Priority::Batch, move |s| {
                let before = progress.clone();
                apply_batch(s, &context, batch, &mut progress, &mut watermark)?;
                crate::archive::totals::add_batch(s,context.source_profile_id,progress.new_urls-before.new_urls,progress.new_visits-before.new_visits,progress.new_downloads-before.new_downloads)?;
                if context.tracks_watermark() {
                    save_watermark(s, &context.profile_id, &watermark)?;
                } else {
                    s.execute("INSERT INTO profile_watermarks(profile_id,last_source_batch_id,updated_at) VALUES(?1,?2,?3) ON CONFLICT(profile_id) DO UPDATE SET last_source_batch_id=excluded.last_source_batch_id",
                        params![context.profile_id.clone(), watermark.last_source_batch_id, watermark.updated_at.clone()])?;
                }
                crate::archive::write_runs::record_progress(
                    s,
                    context.run_id,
                    &[
                        (
                            "processedRecords",
                            progress.visit_count + progress.skipped_visits
                                - before.visit_count
                                - before.skipped_visits,
                        ),
                        ("newVisits", progress.new_visits - before.new_visits),
                        ("newUrls", progress.new_urls - before.new_urls),
                        ("processedUrls", progress.url_count - before.url_count),
                        ("newDownloads", progress.new_downloads - before.new_downloads),
                        ("skippedRecords", progress.skipped_visits - before.skipped_visits),
                    ],
                )?;
                if let Some(origin) = &context.import {
                    let text = s.query_one(
                        "SELECT summary_json FROM import_batches WHERE id=?1",
                        [origin.batch_id],
                        |r| Ok(r.get::<String>(0)?),
                    )?;
                    let mut summary: serde_json::Value = serde_json::from_str(&text)?;
                    for (key, delta) in [
                        (
                            "candidateItems",
                            progress.visit_count + progress.skipped_visits
                                - before.visit_count
                                - before.skipped_visits,
                        ),
                        ("importedItems", progress.new_visits - before.new_visits),
                        (
                            "duplicateItems",
                            (progress.visit_count - before.visit_count)
                                - (progress.new_visits - before.new_visits),
                        ),
                    ] {
                        summary[key] =
                            serde_json::json!(summary[key].as_u64().unwrap_or(0) + delta as u64);
                    }
                    s.execute(
                        "UPDATE import_batches SET summary_json=?2,visible_items=visible_items+?3 WHERE id=?1",
                        params![origin.batch_id, summary.to_string(), (progress.new_visits-before.new_visits) as i64],
                    )?;
                }
                Ok((progress, watermark))
            })?;
            self.progress = next;
            self.watermark = cursor;
            if report_visits && let Some(report) = self.report_progress.as_mut() {
                report(self.progress.event());
            }
            self.cancel.check()?;
        }
        Ok(())
    }
}

impl HistoryBatchConsumer for ArchiveChunkConsumer<'_> {
    type Error = anyhow::Error;
    fn begin_urls(&mut self, scan: browser_history_parser::UrlScan) -> Result<()> {
        self.watermark.url_scan = Some(scan);
        Ok(())
    }
    fn urls(&mut self, rows: Vec<ParsedUrl>) -> Result<()> {
        for chunk in rows.chunks(BATCH_ROWS) {
            self.push(Batch::Urls(hashed(chunk)?))?;
        }
        Ok(())
    }
    fn visits(&mut self, rows: Vec<ParsedVisit>) -> Result<()> {
        for chunk in rows.chunks(BATCH_ROWS) {
            self.push(Batch::Visits(hashed(chunk)?))?;
        }
        Ok(())
    }
    fn downloads(&mut self, rows: Vec<ParsedDownload>) -> Result<()> {
        // File imports preserve non-visit facts as cold evidence; backup promotes them.
        if self.context.import.is_some() {
            return Ok(());
        }
        for chunk in rows.chunks(BATCH_ROWS) {
            self.push(Batch::Downloads(hashed(chunk)?))?;
        }
        Ok(())
    }
    fn search_terms(&mut self, rows: Vec<ParsedSearchTerm>) -> Result<()> {
        // File imports preserve non-visit facts as cold evidence; backup promotes them.
        if self.context.import.is_some() {
            return Ok(());
        }
        for chunk in rows.chunks(BATCH_ROWS) {
            self.push(Batch::Terms(chunk.to_vec()))?;
        }
        Ok(())
    }
    fn favicons(&mut self, rows: Vec<ParsedFavicon>) -> Result<()> {
        // File imports preserve non-visit facts as cold evidence; backup promotes them.
        if self.context.import.is_some() {
            return Ok(());
        }
        for chunk in rows.chunks(BATCH_ROWS) {
            self.push(Batch::Favicons(hashed(chunk)?))?;
        }
        Ok(())
    }
    fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<()> {
        self.cancel.check()?;
        self.evidence.push(crate::archive::SourceEvidencePayload {
            typed_evidence: chunk.typed_evidence,
            native_entities: chunk.native_entities,
        })?;
        self.flush()
    }
    fn retain_source_evidence_in_report(&self) -> bool {
        false
    }
}

fn hashed<T: serde::Serialize + Clone>(rows: &[T]) -> Result<Vec<(T, String)>> {
    rows.iter().map(|row| Ok((row.clone(), sha256_hex(&serde_json::to_vec(row)?)))).collect()
}

fn apply_batch(
    s: &Session<'_>,
    c: &WriteContext,
    batch: Batch,
    p: &mut ArchiveStreamProgress,
    w: &mut Watermark,
) -> Result<()> {
    let mut pages = BTreeSet::new();
    let mut moved_pages = BTreeSet::new();
    let mut added = std::collections::BTreeMap::<i64, (i64, i64)>::new();
    match batch {
        Batch::Urls(rows) => {
            for (mut url, hash) in rows {
                if c.import.is_some() {
                    url.visit_count = url.visit_count.max(1);
                    url.typed_count = url.typed_count.max(0);
                }
                let (_, page, previous_page) =
                    upsert_url(s, c.run_id, c.source_profile_id, &url, &hash)?;
                pages.insert(page);
                if let Some(old_page) = previous_page
                    && old_page != page
                {
                    moved_pages.insert(old_page);
                    moved_pages.insert(page);
                }
                p.new_urls += usize::from(previous_page.is_none());
                p.url_count += 1;
                let marker = url_marker(&c.browser_family, &url);
                w.last_url_last_visit_time = w.last_url_last_visit_time.max(marker);
                if let Some(scan) = &mut w.url_scan {
                    scan.after_id = scan.after_id.max(url.source_url_id);
                }
                // Search terms have their own cursor, so URL progress can commit now.
                p.last_url_marker =
                    Some(p.last_url_marker.unwrap_or(w.last_url_last_visit_time).max(marker));
            }
        }
        Batch::Visits(rows) => {
            let mut bounds = HashMap::new();
            for (mut visit, hash) in rows {
                if p.first_visit_iso.as_ref().is_none_or(|v| v > &visit.visit_time_iso) {
                    p.first_visit_iso = Some(visit.visit_time_iso.clone());
                }
                if p.last_visit_iso.as_ref().is_none_or(|v| v < &visit.visit_time_iso) {
                    p.last_visit_iso = Some(visit.visit_time_iso.clone());
                }
                let Some((id, page)) = resolve_url(s, c.source_profile_id, visit.source_url_id)?
                else {
                    p.skipped_visits += 1;
                    continue;
                };
                let inserted = if let Some(origin) = &c.import {
                    if origin.source_kind == "takeout" {
                        visit.transition = None;
                        visit.visit_duration_ms = None;
                        visit.from_visit = None;
                        visit.is_known_to_sync = false;
                        visit.visited_link_id = None;
                        visit.external_referrer_url = origin.source_path.clone();
                        visit.app_id = Some("takeout".into());
                    }
                    insert_import_visit(
                        s,
                        c.run_id,
                        c.source_profile_id,
                        id,
                        &visit,
                        &hash,
                        origin,
                    )?
                } else {
                    insert_visit(s, c.run_id, c.source_profile_id, id, &visit, &hash)?
                };
                if inserted > 0 {
                    p.new_visits += 1;
                    track_url_visit_bounds(&mut bounds, id, &visit);
                    added
                        .entry(page)
                        .and_modify(|(count, newest)| {
                            *count += 1;
                            *newest = (*newest).max(visit.visit_time_ms);
                        })
                        .or_insert((1, visit.visit_time_ms));
                }
                p.visit_count += 1;
                p.last_visit_id = p.last_visit_id.max(visit.source_visit_id);
                w.last_visit_id = w.last_visit_id.max(visit.source_visit_id);
            }
            for (id, bounds) in bounds {
                sync_url_bounds(s, id, &bounds)?;
            }
        }
        Batch::Downloads(rows) => {
            for (row, hash) in rows {
                p.new_downloads +=
                    insert_download(s, c.run_id, c.source_profile_id, &row, &hash)? as usize;
                p.download_count += 1;
                w.last_download_id = w.last_download_id.max(row.source_download_id);
                p.last_download_id = Some(w.last_download_id);
            }
        }
        Batch::Terms(rows) => {
            for row in rows {
                w.last_search_url_time = w.last_search_url_time.max(row.url_last_visit_time);
                if let Some((id, page)) = resolve_url(s, c.source_profile_id, row.url_id)? {
                    p.inserted_search_terms += insert_search_term(
                        s,
                        c.run_id,
                        c.source_profile_id,
                        &c.profile_id,
                        id,
                        &row,
                    )? as usize;
                    p.search_term_count += 1;
                    pages.insert(page);
                }
            }
        }
        Batch::Favicons(rows) => {
            for (row, hash) in rows {
                insert_favicon(s, c.run_id, c.source_profile_id, &row, &hash)?;
                w.last_favicon_last_updated =
                    w.last_favicon_last_updated.max(favicon_marker(&c.browser_family, &row));
                p.last_favicon_marker = Some(w.last_favicon_last_updated);
            }
        }
    }
    w.updated_at = now_rfc3339();
    for page in &moved_pages {
        crate::archive::search_documents::refresh(s, *page)?;
    }
    for page in pages {
        if !moved_pages.contains(&page) {
            crate::archive::search_documents::text_changed(s, page)?;
        }
    }
    for (page, (count, newest)) in added {
        crate::archive::search_documents::visits_added(
            s,
            page,
            c.source_profile_id,
            count,
            newest,
        )?;
    }
    Ok(())
}

fn resolve_url(
    s: &Session<'_>,
    profile: i64,
    source_id: i64,
) -> vault_store::Result<Option<(i64, i64)>> {
    Ok(s.query(
        "SELECT id,page_id FROM urls WHERE source_profile_id=?1 AND source_url_id=?2",
        params![profile, source_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?
    .pop())
}
#[cfg(test)]
mod statement_benchmark {
    //! Responsibilities: bound SQL work for production-sized URL and visit batches.
    //! Not responsible for: capacity claims or browser parsing throughput.
    //! Failure modes: per-page work grows with archive size, duplicate document refreshes.
    //! Counts include archive facts, projections, watermark, progress and totals;
    //! Store transaction control and the separately committed evidence are excluded.
    use super::*;
    // URL: read prior identity + upsert page + upsert observation (3),
    // read page/title/terms/annotations + write document (5), outbox (1).
    const URL_ROW_BUDGET: usize = 3 + 5 + 1;
    // Visit: resolve URL + insert + update bounds (3), document text (5),
    // profile membership + newest time + outbox (3).
    const VISIT_ROW_BUDGET: usize = 3 + 5 + 3;
    // Watermark (1), run progress read/write (2), three cached totals (3).
    const BATCH_BUDGET: usize = 1 + 2 + 3;
    #[test]
    fn statements_follow_the_row_and_batch_budget() {
        measure(32);
    }
    #[test]
    #[ignore = "scaled measurement; the same budgets run in the normal test suite"]
    fn statements_per_ten_thousand_url_batch() {
        measure(10_000);
    }
    fn measure(size: i64) {
        let vault = Vault::open_temp().unwrap();
        let profile = super::super::core_tests::profile("chromium", "benchmark");
        let count = vault
            .write(Db::Archive, Priority::Batch, move |s| {
                let source_profile_id = upsert_source_profile(s, &profile)?;
                let context = WriteContext {
                    run_id: 0,
                    source_profile_id,
                    profile_id: profile.profile_id,
                    browser_family: "chromium".into(),
                    import: None,
                };
                let rows = (1..=size)
                    .map(|id| {
                        (
                            ParsedUrl {
                                source_url_id: id,
                                url: format!("https://benchmark.test/{id}"),
                                title: Some(format!("Page {id}")),
                                visit_count: 1,
                                typed_count: 0,
                                last_visit_ms: id,
                                last_visit_iso: id.to_string(),
                                hidden: false,
                                source_last_visit_marker: Some(id),
                            },
                            id.to_string(),
                        )
                    })
                    .collect();
                let before = s.statement_count();
                let mut progress = ArchiveStreamProgress::default();
                let mut watermark = Watermark::default();
                apply_batch(s, &context, Batch::Urls(rows), &mut progress, &mut watermark)?;
                save_watermark(s, &context.profile_id, &watermark)?;
                crate::archive::write_runs::record_progress(
                    s,
                    context.run_id,
                    &[
                        ("processedRecords", 0),
                        ("newVisits", 0),
                        ("newUrls", progress.new_urls),
                        ("newDownloads", 0),
                        ("skippedRecords", 0),
                    ],
                )?;
                crate::archive::totals::add_batch(
                    s,
                    context.source_profile_id,
                    progress.new_urls,
                    0,
                    0,
                )?;
                let url_count = s.statement_count() - before;
                let visits = (1..=size)
                    .map(|id| {
                        (
                            ParsedVisit {
                                source_visit_id: id,
                                source_url_id: id,
                                url: format!("https://benchmark.test/{id}"),
                                title: Some(format!("Page {id}")),
                                visit_time_ms: id,
                                visit_time_iso: id.to_string(),
                                from_visit: None,
                                transition: None,
                                visit_duration_ms: None,
                                is_known_to_sync: false,
                                visited_link_id: None,
                                external_referrer_url: None,
                                app_id: None,
                            },
                            id.to_string(),
                        )
                    })
                    .collect();
                let before = s.statement_count();
                apply_batch(s, &context, Batch::Visits(visits), &mut progress, &mut watermark)?;
                save_watermark(s, &context.profile_id, &watermark)?;
                crate::archive::write_runs::record_progress(
                    s,
                    context.run_id,
                    &[("newVisits", progress.new_visits)],
                )?;
                crate::archive::totals::add_batch(
                    s,
                    context.source_profile_id,
                    0,
                    progress.new_visits,
                    0,
                )?;
                Ok((url_count, s.statement_count() - before))
            })
            .unwrap();
        eprintln!("{size} URLs: {} statements; {size} visits: {} statements", count.0, count.1);
        assert!(
            count.0 <= size as usize * URL_ROW_BUDGET + BATCH_BUDGET,
            "URL batch must refresh each document only once: {count:?}"
        );
        assert!(
            count.1 <= size as usize * VISIT_ROW_BUDGET + BATCH_BUDGET,
            "visit batch statement bound exceeded: {count:?}"
        );
    }
}
