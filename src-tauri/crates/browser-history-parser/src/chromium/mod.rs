//! Chromium history parser.
//!
//! This module reads already-staged Chromium `History` and `Favicons`
//! databases. It deliberately avoids browser discovery and live-file copying;
//! those concerns belong to higher layers.

use crate::{
    error::ParseError,
    observation::{capability_snapshot, capture_native_row, inspect_schema},
    types::{
        CapabilityCoverage, ChromiumHistory, ChromiumReadCursor, ContextEvidence,
        DatabaseInspection, EngagementEvidence, HistoryBatchConsumer, HistoryDatabaseSet,
        NavigationEvidence, ParsedDownload, ParsedFavicon, ParsedSearchTerm, ParsedUrl,
        ParsedVisit, ParserWarning, SearchEvidence, SourceEvidenceChunk, StreamHistoryError,
        StreamedHistory, TypedEvidenceBatch,
    },
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
use std::convert::Infallible;
use std::path::Path;

const CHROME_UNIX_EPOCH_OFFSET_MICROS: i64 = 11_644_473_600_000_000;

#[derive(Debug, Default)]
struct ChromiumHistoryCollector {
    urls: Vec<ParsedUrl>,
    visits: Vec<ParsedVisit>,
    downloads: Vec<ParsedDownload>,
    search_terms: Vec<ParsedSearchTerm>,
    favicons: Vec<ParsedFavicon>,
}

impl HistoryBatchConsumer for ChromiumHistoryCollector {
    type Error = Infallible;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.urls.extend(batch);
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.visits.extend(batch);
        Ok(())
    }

    fn downloads(&mut self, batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
        self.downloads.extend(batch);
        Ok(())
    }

    fn search_terms(&mut self, batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
        self.search_terms.extend(batch);
        Ok(())
    }

    fn favicons(&mut self, batch: Vec<ParsedFavicon>) -> Result<(), Self::Error> {
        self.favicons.extend(batch);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct VisitCapabilityStats {
    total_visits: usize,
    from_visit_count: usize,
    external_referrer_count: usize,
    visit_duration_count: usize,
    sync_state_count: usize,
    search_evidence_count: usize,
    engagement_evidence_count: usize,
}

/// Incremental URL ingest query used by the archive pipeline.
pub const INGEST_URLS_SQL: &str =
    "SELECT id, url, title, visit_count, typed_count, last_visit_time, hidden
     FROM urls
     WHERE last_visit_time >= ?1
     ORDER BY last_visit_time ASC";
/// Incremental visit ingest query used by the archive pipeline.
pub const INGEST_VISITS_SQL: &str =
    "SELECT visits.id, visits.url, urls.url, urls.title, visits.visit_time, visits.from_visit,
            visits.transition, visits.visit_duration, visits.is_known_to_sync,
            visits.visited_link_id, visits.external_referrer_url, visits.app_id
     FROM visits
     JOIN urls ON urls.id = visits.url
     WHERE visits.id > ?1
     ORDER BY visits.id ASC";
/// Incremental download ingest query used by the archive pipeline.
pub const DOWNLOADS_SQL: &str =
    "SELECT id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state,
            mime_type, original_mime_type
     FROM downloads
     WHERE id > ?1
     ORDER BY id ASC";
/// Incremental keyword-search-term query used by the archive pipeline.
pub const SEARCH_TERMS_SQL: &str = "SELECT keyword_id, url_id, term, normalized_term
     FROM keyword_search_terms
     WHERE url_id IN (
       SELECT id FROM urls WHERE last_visit_time >= ?1
     )";
/// Incremental favicons query used by the archive pipeline.
pub const FAVICONS_SQL: &str = "SELECT icon_mapping.page_url, favicons.url, favicons.icon_type,
            IFNULL(favicon_bitmaps.width, 0), IFNULL(favicon_bitmaps.height, 0),
            IFNULL(favicon_bitmaps.last_updated, 0), favicon_bitmaps.image_data
     FROM icon_mapping
     JOIN favicons ON favicons.id = icon_mapping.icon_id
     LEFT JOIN favicon_bitmaps ON favicon_bitmaps.icon_id = favicons.id
     WHERE IFNULL(favicon_bitmaps.last_updated, 0) >= ?1
     ORDER BY IFNULL(favicon_bitmaps.last_updated, 0) ASC";

/// Inspects a staged Chromium source and reports table coverage/warnings.
pub fn inspect_history(source: &HistoryDatabaseSet) -> Result<DatabaseInspection, ParseError> {
    let connection = open_readonly(&source.history_path)?;
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut warnings = Vec::new();
    for table_name in ["urls", "visits"] {
        if !table_names.iter().any(|existing| existing == table_name) {
            warnings.push(ParserWarning {
                code: "missing-table".to_string(),
                message: format!("required Chromium table `{table_name}` is missing"),
            });
        }
    }

    Ok(DatabaseInspection { table_names, warnings })
}

/// Parses a staged Chromium source into deterministic parser read models.
pub fn parse_history(
    source: &HistoryDatabaseSet,
    cursor: ChromiumReadCursor,
) -> Result<ChromiumHistory, ParseError> {
    let mut collector = ChromiumHistoryCollector::default();
    let streamed =
        stream_history(source, cursor, 10_000, &mut collector).map_err(|error| match error {
            StreamHistoryError::Parse(error) => error,
            StreamHistoryError::Consumer(never) => match never {},
        })?;
    Ok(ChromiumHistory {
        inspection: streamed.inspection,
        schema_observation: streamed.schema_observation,
        capability_snapshot: streamed.capability_snapshot,
        urls: collector.urls,
        visits: collector.visits,
        downloads: collector.downloads,
        search_terms: collector.search_terms,
        favicons: collector.favicons,
        typed_evidence: streamed.typed_evidence,
        native_entities: streamed.native_entities,
        warnings: streamed.warnings,
    })
}

/// Streams canonical Chromium rows into a caller-provided batch consumer.
///
/// The consumer sees URL/visit/download/search-term/favicon batches as they
/// are parsed, which lets archive ingest start persisting canonical rows before
/// the entire staged source database has been materialized in memory.
pub fn stream_history<C>(
    source: &HistoryDatabaseSet,
    cursor: ChromiumReadCursor,
    chunk_size: usize,
    consumer: &mut C,
) -> Result<StreamedHistory, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let inspection = inspect_history(source)?;
    let schema_observation =
        inspect_schema(&open_readonly(&source.history_path)?, &["urls", "visits"])?;
    validate_required_tables(&inspection)?;

    let history = open_readonly(&source.history_path)?;
    let chunk_size = chunk_size.max(1);
    let mut warnings = inspection.warnings.clone();
    let mut typed_evidence = TypedEvidenceBatch::default();
    let mut native_entities = Vec::new();
    let mut visit_stats = VisitCapabilityStats::default();
    let retain_source_evidence = consumer.retain_source_evidence_in_report();

    stream_url_batches(
        &history,
        cursor.after_url_last_visit_time,
        chunk_size,
        consumer,
        retain_source_evidence,
        &mut typed_evidence,
        &mut native_entities,
    )?;
    stream_visit_batches(
        &history,
        cursor.after_visit_id,
        chunk_size,
        consumer,
        retain_source_evidence,
        &mut typed_evidence,
        &mut visit_stats,
        &mut native_entities,
    )?;

    if has_table(&inspection, "downloads") {
        stream_download_batches(
            &history,
            cursor.after_download_id,
            chunk_size,
            consumer,
            retain_source_evidence,
            &mut typed_evidence,
            &mut native_entities,
        )?;
    } else {
        warnings.push(ParserWarning {
            code: "missing-table".to_string(),
            message: "optional Chromium table `downloads` is missing".to_string(),
        });
    }

    if has_table(&inspection, "keyword_search_terms") {
        stream_search_term_batches(
            &history,
            cursor.after_url_last_visit_time,
            chunk_size,
            consumer,
            retain_source_evidence,
            &mut typed_evidence,
            &mut native_entities,
            &mut visit_stats,
        )?;
    } else {
        warnings.push(ParserWarning {
            code: "missing-table".to_string(),
            message: "optional Chromium table `keyword_search_terms` is missing".to_string(),
        });
    }

    match &source.favicons_path {
        Some(path) => {
            stream_favicon_batches(path, cursor.after_favicon_last_updated, chunk_size, consumer)?;
        }
        None => warnings.push(ParserWarning {
            code: "missing-source".to_string(),
            message: "favicons database was not provided".to_string(),
        }),
    }

    let capability_snapshot = build_capability_snapshot(&inspection, &visit_stats);
    Ok(StreamedHistory {
        inspection,
        schema_observation,
        capability_snapshot,
        typed_evidence,
        native_entities,
        warnings,
    })
}

/// Converts Chromium's microsecond timestamp format to Unix milliseconds.
pub fn chrome_time_to_unix_ms(value: i64) -> i64 {
    value.saturating_sub(CHROME_UNIX_EPOCH_OFFSET_MICROS).div_euclid(1_000).max(0)
}

/// Converts Chromium's microsecond timestamp format to RFC3339.
pub fn chrome_time_to_iso(value: i64) -> String {
    let milliseconds = chrome_time_to_unix_ms(value);
    Utc.timestamp_millis_opt(milliseconds)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .to_rfc3339()
}

fn open_readonly(path: &Path) -> Result<Connection, ParseError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|source| ParseError::OpenDatabase { path: path.to_path_buf(), source })
}

fn inspect_connection_tables(connection: &Connection) -> Result<DatabaseInspection, ParseError> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(DatabaseInspection { table_names, warnings: Vec::new() })
}

fn stream_sql<T, E>(result: Result<T, rusqlite::Error>) -> Result<T, StreamHistoryError<E>> {
    result.map_err(ParseError::from).map_err(StreamHistoryError::Parse)
}

fn validate_required_tables(inspection: &DatabaseInspection) -> Result<(), ParseError> {
    for table_name in ["urls", "visits"] {
        if !has_table(inspection, table_name) {
            return Err(ParseError::MissingTable { table: table_name });
        }
    }
    Ok(())
}

fn has_table(inspection: &DatabaseInspection, table_name: &str) -> bool {
    inspection.table_names.iter().any(|existing| existing == table_name)
}

fn flush_source_evidence<C>(
    consumer: &mut C,
    retain_source_evidence: bool,
    typed_evidence: &mut TypedEvidenceBatch,
    native_entities: &mut Vec<crate::types::NativeEntity>,
    chunk: &mut SourceEvidenceChunk,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    if chunk.is_empty() {
        return Ok(());
    }

    let chunk = std::mem::take(chunk);
    if retain_source_evidence {
        typed_evidence.search.extend(chunk.typed_evidence.search);
        typed_evidence.navigation.extend(chunk.typed_evidence.navigation);
        typed_evidence.engagement.extend(chunk.typed_evidence.engagement);
        typed_evidence.context.extend(chunk.typed_evidence.context);
        native_entities.extend(chunk.native_entities);
    } else {
        consumer.source_evidence(chunk).map_err(StreamHistoryError::Consumer)?;
    }
    Ok(())
}

fn stream_url_batches<C>(
    connection: &Connection,
    last_visit_time: i64,
    chunk_size: usize,
    consumer: &mut C,
    retain_source_evidence: bool,
    typed_evidence: &mut TypedEvidenceBatch,
    native_entities: &mut Vec<crate::types::NativeEntity>,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let mut statement = stream_sql(connection.prepare(INGEST_URLS_SQL))?;
    let column_names =
        statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let mut rows = stream_sql(statement.query(params![last_visit_time]))?;
    let mut batch = Vec::with_capacity(chunk_size);
    let mut source_evidence = SourceEvidenceChunk::default();
    while let Some(row) = stream_sql(rows.next())? {
        batch.push(stream_sql(parsed_url_from_row(row))?);
        source_evidence.native_entities.push(stream_sql(capture_native_row(
            row,
            &column_names,
            "chromium-url-row",
            "id",
            None,
        ))?);
        if batch.len() >= chunk_size {
            consumer.urls(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
            flush_source_evidence(
                consumer,
                retain_source_evidence,
                typed_evidence,
                native_entities,
                &mut source_evidence,
            )?;
        }
    }
    if !batch.is_empty() {
        consumer.urls(batch).map_err(StreamHistoryError::Consumer)?;
    }
    flush_source_evidence(
        consumer,
        retain_source_evidence,
        typed_evidence,
        native_entities,
        &mut source_evidence,
    )?;
    Ok(())
}

fn stream_visit_batches<C>(
    connection: &Connection,
    last_visit_id: i64,
    chunk_size: usize,
    consumer: &mut C,
    retain_source_evidence: bool,
    typed_evidence: &mut TypedEvidenceBatch,
    visit_stats: &mut VisitCapabilityStats,
    native_entities: &mut Vec<crate::types::NativeEntity>,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let mut statement = stream_sql(connection.prepare(INGEST_VISITS_SQL))?;
    let column_names =
        statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let mut rows = stream_sql(statement.query(params![last_visit_id]))?;
    let mut batch = Vec::with_capacity(chunk_size);
    let mut source_evidence = SourceEvidenceChunk::default();
    while let Some(row) = stream_sql(rows.next())? {
        let visit = stream_sql(parsed_visit_from_row(row))?;
        track_visit_capability_stats(visit_stats, &visit);
        if let Some(evidence) = navigation_evidence_for_visit(&visit) {
            source_evidence.typed_evidence.navigation.push(evidence);
        }
        if let Some(evidence) = engagement_evidence_for_visit(&visit) {
            visit_stats.engagement_evidence_count += 1;
            source_evidence.typed_evidence.engagement.push(evidence);
        }
        source_evidence.typed_evidence.context.extend(context_evidence_for_visit(&visit));
        source_evidence.native_entities.push(stream_sql(capture_native_row(
            row,
            &column_names,
            "chromium-visit-row",
            "id",
            Some("url"),
        ))?);
        batch.push(visit);
        if batch.len() >= chunk_size {
            consumer.visits(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
            flush_source_evidence(
                consumer,
                retain_source_evidence,
                typed_evidence,
                native_entities,
                &mut source_evidence,
            )?;
        }
    }
    if !batch.is_empty() {
        consumer.visits(batch).map_err(StreamHistoryError::Consumer)?;
    }
    flush_source_evidence(
        consumer,
        retain_source_evidence,
        typed_evidence,
        native_entities,
        &mut source_evidence,
    )?;
    Ok(())
}

fn stream_download_batches<C>(
    connection: &Connection,
    last_download_id: i64,
    chunk_size: usize,
    consumer: &mut C,
    retain_source_evidence: bool,
    typed_evidence: &mut TypedEvidenceBatch,
    native_entities: &mut Vec<crate::types::NativeEntity>,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let mut statement = stream_sql(connection.prepare(DOWNLOADS_SQL))?;
    let column_names =
        statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let mut rows = stream_sql(statement.query(params![last_download_id]))?;
    let mut batch = Vec::with_capacity(chunk_size);
    let mut source_evidence = SourceEvidenceChunk::default();
    while let Some(row) = stream_sql(rows.next())? {
        batch.push(stream_sql(parsed_download_from_row(row))?);
        source_evidence.native_entities.push(stream_sql(capture_native_row(
            row,
            &column_names,
            "chromium-download-row",
            "id",
            None,
        ))?);
        if batch.len() >= chunk_size {
            consumer.downloads(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
            flush_source_evidence(
                consumer,
                retain_source_evidence,
                typed_evidence,
                native_entities,
                &mut source_evidence,
            )?;
        }
    }
    if !batch.is_empty() {
        consumer.downloads(batch).map_err(StreamHistoryError::Consumer)?;
    }
    flush_source_evidence(
        consumer,
        retain_source_evidence,
        typed_evidence,
        native_entities,
        &mut source_evidence,
    )?;
    Ok(())
}

fn stream_search_term_batches<C>(
    connection: &Connection,
    last_visit_time: i64,
    chunk_size: usize,
    consumer: &mut C,
    retain_source_evidence: bool,
    typed_evidence: &mut TypedEvidenceBatch,
    native_entities: &mut Vec<crate::types::NativeEntity>,
    visit_stats: &mut VisitCapabilityStats,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let mut statement = stream_sql(connection.prepare(SEARCH_TERMS_SQL))?;
    let column_names =
        statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let mut rows = stream_sql(statement.query(params![last_visit_time]))?;
    let mut batch = Vec::with_capacity(chunk_size);
    let mut source_evidence = SourceEvidenceChunk::default();
    while let Some(row) = stream_sql(rows.next())? {
        let term = stream_sql(parsed_search_term_from_row(row))?;
        source_evidence.typed_evidence.search.push(SearchEvidence {
            source_visit_id: None,
            source_url_id: Some(term.url_id),
            evidence_key: "search.native_terms".to_string(),
            evidence_value: term.term.clone(),
            normalized_value: Some(term.normalized_term.clone()),
            source_field: "keyword_search_terms.term".to_string(),
        });
        visit_stats.search_evidence_count += 1;
        source_evidence.native_entities.push(stream_sql(capture_native_row(
            row,
            &column_names,
            "chromium-search-term-row",
            "url_id",
            None,
        ))?);
        batch.push(term);
        if batch.len() >= chunk_size {
            consumer
                .search_terms(std::mem::take(&mut batch))
                .map_err(StreamHistoryError::Consumer)?;
            flush_source_evidence(
                consumer,
                retain_source_evidence,
                typed_evidence,
                native_entities,
                &mut source_evidence,
            )?;
        }
    }
    if !batch.is_empty() {
        consumer.search_terms(batch).map_err(StreamHistoryError::Consumer)?;
    }
    flush_source_evidence(
        consumer,
        retain_source_evidence,
        typed_evidence,
        native_entities,
        &mut source_evidence,
    )?;
    Ok(())
}

fn stream_favicon_batches<C>(
    favicons_path: &Path,
    last_favicon_last_updated: i64,
    chunk_size: usize,
    consumer: &mut C,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let connection = open_readonly(favicons_path)?;
    let inspection = inspect_connection_tables(&connection)?;
    if !has_table(&inspection, "favicons")
        || !has_table(&inspection, "icon_mapping")
        || !has_table(&inspection, "favicon_bitmaps")
    {
        return Ok(());
    }

    let mut statement = stream_sql(connection.prepare(FAVICONS_SQL))?;
    let mut rows = stream_sql(statement.query(params![last_favicon_last_updated]))?;
    let mut batch = Vec::with_capacity(chunk_size);
    while let Some(row) = stream_sql(rows.next())? {
        batch.push(stream_sql(parsed_favicon_from_row(row))?);
        if batch.len() >= chunk_size {
            consumer.favicons(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
        }
    }
    if !batch.is_empty() {
        consumer.favicons(batch).map_err(StreamHistoryError::Consumer)?;
    }
    Ok(())
}

fn track_visit_capability_stats(stats: &mut VisitCapabilityStats, visit: &ParsedVisit) {
    stats.total_visits += 1;
    if visit.from_visit.is_some() {
        stats.from_visit_count += 1;
    }
    if visit.external_referrer_url.is_some() {
        stats.external_referrer_count += 1;
    }
    if visit.visit_duration_ms.is_some() {
        stats.visit_duration_count += 1;
    }
    if visit.is_known_to_sync {
        stats.sync_state_count += 1;
    }
}

fn navigation_evidence_for_visit(visit: &ParsedVisit) -> Option<NavigationEvidence> {
    (visit.from_visit.is_some()
        || visit.external_referrer_url.is_some()
        || visit.transition.is_some())
    .then(|| NavigationEvidence {
        source_visit_id: visit.source_visit_id,
        edge_kind: "visit-navigation".to_string(),
        target_visit_id: visit.from_visit,
        target_url: visit.external_referrer_url.clone(),
        transition: visit.transition,
        source_field: "visits.from_visit/external_referrer_url/transition".to_string(),
    })
}

fn engagement_evidence_for_visit(visit: &ParsedVisit) -> Option<EngagementEvidence> {
    visit.visit_duration_ms.map(|value| EngagementEvidence {
        source_visit_id: visit.source_visit_id,
        metric_key: "engagement.visit_duration_ms".to_string(),
        metric_value_int: Some(value),
        metric_value_real: None,
        source_field: "visits.visit_duration".to_string(),
    })
}

fn context_evidence_for_visit(visit: &ParsedVisit) -> Vec<ContextEvidence> {
    let mut items = Vec::new();
    if visit.app_id.is_some() {
        items.push(ContextEvidence {
            source_visit_id: Some(visit.source_visit_id),
            source_url_id: Some(visit.source_url_id),
            context_key: "context.app_id".to_string(),
            value_json: serde_json::json!(visit.app_id).to_string(),
            source_field: "visits.app_id".to_string(),
        });
    }
    if visit.is_known_to_sync {
        items.push(ContextEvidence {
            source_visit_id: Some(visit.source_visit_id),
            source_url_id: Some(visit.source_url_id),
            context_key: "context.is_known_to_sync".to_string(),
            value_json: "true".to_string(),
            source_field: "visits.is_known_to_sync".to_string(),
        });
    }
    if visit.visited_link_id.is_some() {
        items.push(ContextEvidence {
            source_visit_id: Some(visit.source_visit_id),
            source_url_id: Some(visit.source_url_id),
            context_key: "context.visited_link_id".to_string(),
            value_json: serde_json::json!(visit.visited_link_id).to_string(),
            source_field: "visits.visited_link_id".to_string(),
        });
    }
    items
}

fn build_capability_snapshot(
    inspection: &DatabaseInspection,
    visit_stats: &VisitCapabilityStats,
) -> crate::types::CapabilitySnapshot {
    capability_snapshot(vec![
        CapabilityCoverage {
            key: "search.native_terms".to_string(),
            available: has_table(inspection, "keyword_search_terms"),
            populated_rows: visit_stats.search_evidence_count,
            total_rows: visit_stats.search_evidence_count,
            notes: vec!["Chromium keyword_search_terms table".to_string()],
        },
        CapabilityCoverage {
            key: "nav.from_visit".to_string(),
            available: visit_stats.from_visit_count > 0,
            populated_rows: visit_stats.from_visit_count,
            total_rows: visit_stats.total_visits,
            notes: vec!["Chromium visits.from_visit".to_string()],
        },
        CapabilityCoverage {
            key: "nav.external_referrer".to_string(),
            available: visit_stats.external_referrer_count > 0,
            populated_rows: visit_stats.external_referrer_count,
            total_rows: visit_stats.total_visits,
            notes: vec!["Chromium visits.external_referrer_url".to_string()],
        },
        CapabilityCoverage {
            key: "engagement.visit_duration_ms".to_string(),
            available: visit_stats.visit_duration_count > 0,
            populated_rows: visit_stats.engagement_evidence_count,
            total_rows: visit_stats.total_visits,
            notes: vec!["Chromium visits.visit_duration".to_string()],
        },
        CapabilityCoverage {
            key: "context.sync_state".to_string(),
            available: visit_stats.sync_state_count > 0,
            populated_rows: visit_stats.sync_state_count,
            total_rows: visit_stats.total_visits,
            notes: vec!["Chromium visits.is_known_to_sync".to_string()],
        },
    ])
}

fn parsed_url_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedUrl> {
    let last_visit_time = row.get::<_, i64>(5)?;
    Ok(ParsedUrl {
        source_url_id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get(3)?,
        typed_count: row.get(4)?,
        last_visit_ms: chrome_time_to_unix_ms(last_visit_time),
        last_visit_iso: chrome_time_to_iso(last_visit_time),
        hidden: row.get::<_, i64>(6)? != 0,
    })
}

fn parsed_visit_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedVisit> {
    let visit_time = row.get::<_, i64>(4)?;
    Ok(ParsedVisit {
        source_visit_id: row.get(0)?,
        source_url_id: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        visit_time_ms: chrome_time_to_unix_ms(visit_time),
        visit_time_iso: chrome_time_to_iso(visit_time),
        from_visit: row.get(5)?,
        transition: row.get(6)?,
        visit_duration_ms: row.get(7)?,
        is_known_to_sync: row.get::<_, Option<i64>>(8)?.unwrap_or_default() != 0,
        visited_link_id: row.get(9)?,
        external_referrer_url: row.get(10)?,
        app_id: row.get(11)?,
    })
}

fn parsed_download_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedDownload> {
    let start_time = row.get::<_, Option<i64>>(4)?;
    Ok(ParsedDownload {
        source_download_id: row.get(0)?,
        guid: row.get(1)?,
        current_path: row.get(2)?,
        target_path: row.get(3)?,
        start_time_ms: start_time.map(chrome_time_to_unix_ms),
        start_time_iso: start_time.map(chrome_time_to_iso),
        received_bytes: row.get(5)?,
        total_bytes: row.get(6)?,
        state: row.get(7)?,
        mime_type: row.get(8)?,
        original_mime_type: row.get(9)?,
    })
}

fn parsed_search_term_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedSearchTerm> {
    Ok(ParsedSearchTerm {
        keyword_id: row.get(0)?,
        url_id: row.get(1)?,
        term: row.get(2)?,
        normalized_term: row.get(3)?,
    })
}

fn parsed_favicon_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedFavicon> {
    let last_updated = row.get::<_, i64>(5)?;
    Ok(ParsedFavicon {
        page_url: row.get(0)?,
        icon_url: row.get(1)?,
        icon_type: row.get(2)?,
        width: row.get(3)?,
        height: row.get(4)?,
        last_updated_ms: chrome_time_to_unix_ms(last_updated),
        last_updated_iso: chrome_time_to_iso(last_updated),
        image_data: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use tempfile::tempdir;

    #[derive(Default)]
    struct NonRetainingEvidenceSink {
        urls: usize,
        visits: usize,
        downloads: usize,
        search_terms: usize,
        source_evidence_chunks: usize,
        search_evidence: usize,
        engagement_evidence: usize,
        native_entities: usize,
    }

    impl HistoryBatchConsumer for NonRetainingEvidenceSink {
        type Error = Infallible;

        fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            self.urls += batch.len();
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits += batch.len();
            Ok(())
        }

        fn downloads(&mut self, batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
            self.downloads += batch.len();
            Ok(())
        }

        fn search_terms(&mut self, batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
            self.search_terms += batch.len();
            Ok(())
        }

        fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<(), Self::Error> {
            self.source_evidence_chunks += 1;
            self.search_evidence += chunk.typed_evidence.search.len();
            self.engagement_evidence += chunk.typed_evidence.engagement.len();
            self.native_entities += chunk.native_entities.len();
            Ok(())
        }

        fn retain_source_evidence_in_report(&self) -> bool {
            false
        }
    }

    fn write_history_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open history fixture");
        connection
            .execute_batch(
                "CREATE TABLE urls (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER NOT NULL,
                   typed_count INTEGER NOT NULL,
                   last_visit_time INTEGER NOT NULL,
                   hidden INTEGER NOT NULL
                 );
                 CREATE TABLE visits (
                   id INTEGER PRIMARY KEY,
                   url INTEGER NOT NULL,
                   visit_time INTEGER NOT NULL,
                   from_visit INTEGER,
                   transition INTEGER,
                   visit_duration INTEGER,
                   is_known_to_sync INTEGER,
                   visited_link_id INTEGER,
                   external_referrer_url TEXT,
                   app_id TEXT
                 );
                 CREATE TABLE downloads (
                   id INTEGER PRIMARY KEY,
                   guid TEXT,
                   current_path TEXT,
                   target_path TEXT,
                   start_time INTEGER,
                   received_bytes INTEGER,
                   total_bytes INTEGER,
                   state INTEGER,
                   mime_type TEXT,
                   original_mime_type TEXT
                 );
                 CREATE TABLE keyword_search_terms (
                   keyword_id INTEGER,
                   url_id INTEGER,
                   term TEXT,
                   normalized_term TEXT
                 );",
            )
            .expect("create history schema");
        connection
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    1_i64,
                    "https://example.com/article",
                    "Example Article",
                    5_i64,
                    2_i64,
                    13_000_000_000_000_000_i64,
                    0_i64
                ],
            )
            .expect("insert url");
        connection
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    10_i64,
                    1_i64,
                    13_000_000_000_500_000_i64,
                    Option::<i64>::None,
                    805_306_368_i64,
                    4000_i64,
                    1_i64,
                    7_i64,
                    "https://referrer.example.com",
                    "com.example.browser"
                ],
            )
            .expect("insert visit");
        connection
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    4_i64,
                    "download-guid",
                    "/tmp/example.part",
                    "/tmp/example.zip",
                    13_000_000_001_000_000_i64,
                    128_i64,
                    256_i64,
                    1_i64,
                    "application/zip",
                    "application/octet-stream"
                ],
            )
            .expect("insert download");
        connection
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (?1, ?2, ?3, ?4)",
                params![2_i64, 1_i64, "PathKeep", "pathkeep"],
            )
            .expect("insert search term");
    }

    fn write_favicons_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open favicons fixture");
        connection
            .execute_batch(
                "CREATE TABLE favicons (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   icon_type INTEGER
                 );
                 CREATE TABLE icon_mapping (
                   page_url TEXT NOT NULL,
                   icon_id INTEGER NOT NULL
                 );
                 CREATE TABLE favicon_bitmaps (
                   icon_id INTEGER NOT NULL,
                   width INTEGER,
                   height INTEGER,
                   last_updated INTEGER,
                   image_data BLOB
                 );",
            )
            .expect("create favicons schema");
        connection
            .execute(
                "INSERT INTO favicons (id, url, icon_type) VALUES (?1, ?2, ?3)",
                params![3_i64, "https://example.com/favicon.ico", 1_i64],
            )
            .expect("insert favicon");
        connection
            .execute(
                "INSERT INTO icon_mapping (page_url, icon_id) VALUES (?1, ?2)",
                params!["https://example.com/article", 3_i64],
            )
            .expect("insert mapping");
        connection
            .execute(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![3_i64, 32_i64, 32_i64, 13_000_000_002_000_000_i64, vec![1_u8, 2, 3]],
            )
            .expect("insert bitmap");
    }

    fn write_partial_favicons_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open partial favicons fixture");
        connection
            .execute_batch(
                "CREATE TABLE favicons (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   icon_type INTEGER
                 );
                 CREATE TABLE favicon_bitmaps (
                   icon_id INTEGER NOT NULL,
                   width INTEGER,
                   height INTEGER,
                   last_updated INTEGER,
                   image_data BLOB
                 );",
            )
            .expect("create partial favicons schema");
        connection
            .execute(
                "INSERT INTO favicons (id, url, icon_type) VALUES (?1, ?2, ?3)",
                params![3_i64, "https://example.com/favicon.ico", 1_i64],
            )
            .expect("insert partial favicon");
        connection
            .execute(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![3_i64, 32_i64, 32_i64, 13_000_000_002_000_000_i64, vec![1_u8, 2, 3]],
            )
            .expect("insert partial bitmap");
    }

    #[test]
    fn parse_history_returns_incremental_rows_from_provided_paths() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let favicons_path = directory.path().join("Favicons");
        write_history_fixture(&history_path);
        write_favicons_fixture(&favicons_path);

        let parsed = parse_history(
            &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
            ChromiumReadCursor::default(),
        )
        .expect("parse history");

        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
        assert_eq!(parsed.downloads.len(), 1);
        assert_eq!(parsed.search_terms.len(), 1);
        assert_eq!(parsed.favicons.len(), 1);
        assert_eq!(parsed.urls[0].url, "https://example.com/article");
        assert!(!parsed.urls[0].hidden);
        assert_eq!(parsed.search_terms[0].normalized_term, "pathkeep");
        assert!(parsed.visits[0].is_known_to_sync);
        assert_eq!(parsed.favicons[0].width, 32);
    }

    #[test]
    fn stream_history_can_move_source_evidence_out_of_the_returned_report() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let favicons_path = directory.path().join("Favicons");
        write_history_fixture(&history_path);
        write_favicons_fixture(&favicons_path);
        let mut sink = NonRetainingEvidenceSink::default();

        let streamed = stream_history(
            &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
            ChromiumReadCursor::default(),
            1,
            &mut sink,
        )
        .expect("stream history");

        assert_eq!(sink.urls, 1);
        assert_eq!(sink.visits, 1);
        assert_eq!(sink.downloads, 1);
        assert_eq!(sink.search_terms, 1);
        assert!(sink.source_evidence_chunks >= 4);
        assert_eq!(sink.search_evidence, 1);
        assert_eq!(sink.engagement_evidence, 1);
        assert!(sink.native_entities >= 4);
        assert!(streamed.native_entities.is_empty());
        assert!(streamed.typed_evidence.search.is_empty());
        assert!(streamed.typed_evidence.navigation.is_empty());
        assert!(streamed.typed_evidence.engagement.is_empty());
        assert!(streamed.typed_evidence.context.is_empty());
        assert!(
            streamed
                .capability_snapshot
                .items
                .iter()
                .any(|item| { item.key == "search.native_terms" && item.populated_rows == 1 })
        );
    }

    #[test]
    fn inspect_history_reports_missing_required_tables_as_warnings() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute("CREATE TABLE downloads (id INTEGER PRIMARY KEY)", [])
            .expect("create downloads");

        let inspection = inspect_history(&HistoryDatabaseSet { history_path, favicons_path: None })
            .expect("inspect history");

        assert!(inspection.warnings.iter().any(|warning| {
            warning.message.contains("required Chromium table `urls` is missing")
        }));
    }

    #[test]
    fn parse_history_requires_urls_and_visits_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "CREATE TABLE urls (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER NOT NULL,
                   typed_count INTEGER NOT NULL,
                   last_visit_time INTEGER NOT NULL,
                   hidden INTEGER NOT NULL
                 )",
                [],
            )
            .expect("create urls table");

        let error = parse_history(
            &HistoryDatabaseSet { history_path, favicons_path: None },
            ChromiumReadCursor::default(),
        )
        .expect_err("missing visits should fail");
        assert!(matches!(error, ParseError::MissingTable { table: "visits" }));
    }

    #[test]
    fn parse_history_skips_favicons_when_support_tables_are_missing() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let favicons_path = directory.path().join("Favicons");
        write_history_fixture(&history_path);
        write_partial_favicons_fixture(&favicons_path);

        let parsed = parse_history(
            &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
            ChromiumReadCursor::default(),
        )
        .expect("parse history");

        assert!(parsed.favicons.is_empty());
        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
    }

    #[test]
    fn chrome_time_helpers_clamp_invalid_values_and_keep_iso_stable() {
        assert_eq!(chrome_time_to_unix_ms(i64::MIN), 0);
        assert_eq!(chrome_time_to_iso(i64::MIN), "1970-01-01T00:00:00+00:00");
        assert!(chrome_time_to_iso(13_000_000_000_000_000_i64).starts_with("2012-"));
    }
}
