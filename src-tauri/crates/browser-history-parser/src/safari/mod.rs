//! Safari history parser.
//!
//! This slice reads already-staged `History.db` files and extracts visits/URLs.
//! It does not attempt broader Safari artifact coverage; the goal is a
//! trustworthy baseline parser, not speculative inference.

use crate::{
    ParseError, ParsedHistory,
    observation::{capability_snapshot, capture_native_row, capture_native_rows, inspect_schema},
    types::{
        CapabilityCoverage, ContextEvidence, DatabaseInspection, EngagementEvidence,
        HistoryBatchConsumer, NavigationEvidence, ParsedUrl, ParsedVisit, ParserWarning,
        SchemaObservation, SourceEvidenceChunk, StreamHistoryError, StreamedHistory,
        TypedEvidenceBatch,
    },
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
use serde_json::json;
use std::convert::Infallible;
use std::path::Path;

const INSPECT_TABLES_SQL: &str = "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
const URLS_SQL: &str = r#"
SELECT
  history_items.id,
  history_items.url,
  (
    SELECT history_visits.title
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
      AND history_visits.title IS NOT NULL
    ORDER BY history_visits.visit_time DESC, history_visits.id DESC
    LIMIT 1
  ) AS title,
  (
    SELECT COUNT(*)
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
  ) AS visit_count,
  (
    SELECT MAX(history_visits.visit_time)
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
  ) AS last_visit_time
FROM history_items
WHERE EXISTS (
  SELECT 1
  FROM history_visits
  WHERE history_visits.history_item = history_items.id
)
  AND (
    SELECT MAX(history_visits.visit_time)
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
  ) >= ?1
ORDER BY last_visit_time ASC
"#;
const SAFARI_UNIX_EPOCH_OFFSET_SECONDS: f64 = 978_307_200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafariVisitExtraColumn {
    LoadSuccessful,
    HttpNonGet,
    Synthesized,
    RedirectSource,
    RedirectDestination,
    Origin,
    Generation,
    Attributes,
    Score,
}

impl SafariVisitExtraColumn {
    fn column_name(self) -> &'static str {
        match self {
            Self::LoadSuccessful => "load_successful",
            Self::HttpNonGet => "http_non_get",
            Self::Synthesized => "synthesized",
            Self::RedirectSource => "redirect_source",
            Self::RedirectDestination => "redirect_destination",
            Self::Origin => "origin",
            Self::Generation => "generation",
            Self::Attributes => "attributes",
            Self::Score => "score",
        }
    }

    fn source_field(self) -> String {
        format!("history_visits.{}", self.column_name())
    }
}

#[derive(Debug, Default)]
struct SafariVisitEvidenceStats {
    redirect_edges: usize,
    load_outcomes: usize,
    http_methods: usize,
    synthesized_rows: usize,
    scores: usize,
}

#[derive(Debug, Default)]
struct SafariHistoryCollector {
    urls: Vec<ParsedUrl>,
    visits: Vec<ParsedVisit>,
}

impl HistoryBatchConsumer for SafariHistoryCollector {
    type Error = Infallible;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.urls.extend(batch);
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.visits.extend(batch);
        Ok(())
    }
}

fn stream_sql<T, E>(result: Result<T, rusqlite::Error>) -> Result<T, StreamHistoryError<E>> {
    result.map_err(ParseError::from).map_err(StreamHistoryError::Parse)
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
        merge_typed_evidence(typed_evidence, chunk.typed_evidence);
        native_entities.extend(chunk.native_entities);
    } else {
        consumer.source_evidence(chunk).map_err(StreamHistoryError::Consumer)?;
    }
    Ok(())
}

fn merge_typed_evidence(target: &mut TypedEvidenceBatch, next: TypedEvidenceBatch) {
    target.search.extend(next.search);
    target.navigation.extend(next.navigation);
    target.engagement.extend(next.engagement);
    target.context.extend(next.context);
}

fn stream_native_table_source_evidence<C>(
    connection: &Connection,
    sql: &str,
    entity_kind: &str,
    chunk_size: usize,
    consumer: &mut C,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let mut statement = stream_sql(connection.prepare(sql))?;
    let column_names =
        statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let mut rows = stream_sql(statement.query([]))?;
    let mut chunk = SourceEvidenceChunk::default();
    while let Some(row) = stream_sql(rows.next())? {
        chunk.native_entities.push(stream_sql(capture_native_row(
            row,
            &column_names,
            entity_kind,
            "id",
            None,
        ))?);
        if chunk.native_entities.len() >= chunk_size {
            consumer
                .source_evidence(std::mem::take(&mut chunk))
                .map_err(StreamHistoryError::Consumer)?;
        }
    }
    if !chunk.is_empty() {
        consumer.source_evidence(chunk).map_err(StreamHistoryError::Consumer)?;
    }
    Ok(())
}

/// Inspects a Safari `History.db` file and reports required-table coverage.
pub fn inspect_history(path: &Path) -> Result<DatabaseInspection, ParseError> {
    let connection = open_readonly(path)?;
    let mut statement = connection.prepare(INSPECT_TABLES_SQL)?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut warnings = Vec::new();
    for table_name in ["history_items", "history_visits"] {
        if !table_names.iter().any(|existing| existing == table_name) {
            warnings.push(ParserWarning {
                code: "missing-table".to_string(),
                message: format!("required Safari table `{table_name}` is missing"),
            });
        }
    }

    warnings.push(ParserWarning {
        code: "baseline-support".to_string(),
        message:
            "Safari baseline ingest captures history visits only. Full Disk Access may still be required before the desktop app can stage History.db."
                .to_string(),
    });

    Ok(DatabaseInspection { table_names, warnings })
}

/// Parses a Safari `History.db` file into parser read models.
pub fn parse_history(
    path: &Path,
    after_visit_id: i64,
    after_url_last_visit_ms: i64,
) -> Result<ParsedHistory, ParseError> {
    let mut collector = SafariHistoryCollector::default();
    let streamed =
        stream_history(path, after_visit_id, after_url_last_visit_ms, 10_000, &mut collector)
            .map_err(|error| match error {
                StreamHistoryError::Parse(error) => error,
                StreamHistoryError::Consumer(never) => match never {},
            })?;
    Ok(ParsedHistory {
        inspection: streamed.inspection,
        schema_observation: streamed.schema_observation,
        capability_snapshot: streamed.capability_snapshot,
        urls: collector.urls,
        visits: collector.visits,
        downloads: Vec::new(),
        search_terms: Vec::new(),
        favicons: Vec::new(),
        typed_evidence: streamed.typed_evidence,
        native_entities: streamed.native_entities,
        warnings: streamed.warnings,
    })
}

/// Streams Safari URL and visit rows into a caller-provided batch consumer.
pub fn stream_history<C>(
    path: &Path,
    after_visit_id: i64,
    after_url_last_visit_ms: i64,
    chunk_size: usize,
    consumer: &mut C,
) -> Result<StreamedHistory, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let inspection = inspect_history(path)?;
    let schema_observation =
        inspect_schema(&open_readonly(path)?, &["history_items", "history_visits"])?;
    let extra_visit_columns = safari_visit_extra_columns(&schema_observation);
    validate_required_tables(&inspection)?;

    let connection = open_readonly(path)?;
    let chunk_size = chunk_size.max(1);
    let warnings = inspection.warnings.clone();
    let mut visit_count = 0usize;
    let mut typed_evidence = TypedEvidenceBatch::default();
    let mut evidence_stats = SafariVisitEvidenceStats::default();
    let mut native_entities = Vec::new();
    let retain_source_evidence = consumer.retain_source_evidence_in_report();

    {
        let mut statement = stream_sql(connection.prepare(URLS_SQL))?;
        let column_names =
            statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let mut rows =
            stream_sql(statement.query(params![unix_ms_to_safari_time(after_url_last_visit_ms)]))?;
        let mut batch = Vec::with_capacity(chunk_size);
        let mut source_evidence = SourceEvidenceChunk::default();
        while let Some(row) = stream_sql(rows.next())? {
            batch.push(stream_sql(parsed_url_from_row(row))?);
            source_evidence.native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "safari-history-item-row",
                "id",
                None,
            ))?);
            if batch.len() >= chunk_size {
                consumer.urls(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
                flush_source_evidence(
                    consumer,
                    retain_source_evidence,
                    &mut typed_evidence,
                    &mut native_entities,
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
            &mut typed_evidence,
            &mut native_entities,
            &mut source_evidence,
        )?;
    }

    {
        let visits_sql = safari_visits_sql(&extra_visit_columns);
        let mut statement = stream_sql(connection.prepare(&visits_sql))?;
        let column_names =
            statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let mut rows = stream_sql(statement.query(params![after_visit_id]))?;
        let mut batch = Vec::with_capacity(chunk_size);
        let mut source_evidence = SourceEvidenceChunk::default();
        while let Some(row) = stream_sql(rows.next())? {
            let visit = stream_sql(parsed_visit_from_row(row))?;
            visit_count += 1;
            let mut visit_typed_evidence = TypedEvidenceBatch::default();
            extend_typed_evidence_from_visit_row(
                row,
                &visit,
                &extra_visit_columns,
                &mut visit_typed_evidence,
                &mut evidence_stats,
            )
            .map_err(ParseError::from)
            .map_err(StreamHistoryError::Parse)?;
            merge_typed_evidence(&mut source_evidence.typed_evidence, visit_typed_evidence);
            source_evidence.native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "safari-history-visit-row",
                "id",
                Some("history_item"),
            ))?);
            batch.push(visit);
            if batch.len() >= chunk_size {
                consumer
                    .visits(std::mem::take(&mut batch))
                    .map_err(StreamHistoryError::Consumer)?;
                flush_source_evidence(
                    consumer,
                    retain_source_evidence,
                    &mut typed_evidence,
                    &mut native_entities,
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
            &mut typed_evidence,
            &mut native_entities,
            &mut source_evidence,
        )?;
    }

    for optional_table in ["history_tombstones", "history_tags", "history_items_to_tags"] {
        if inspection.table_names.iter().any(|existing| existing == optional_table) {
            let sql = format!("SELECT * FROM {optional_table}");
            let entity_kind = format!("safari-{}", optional_table.replace('_', "-"));
            if retain_source_evidence {
                native_entities.extend(capture_native_rows(
                    &connection,
                    &sql,
                    &[],
                    &entity_kind,
                    "id",
                    None,
                )?);
            } else {
                stream_native_table_source_evidence(
                    &connection,
                    &sql,
                    &entity_kind,
                    chunk_size,
                    consumer,
                )?;
            }
        }
    }

    let capability_snapshot = build_capability_snapshot(visit_count, &evidence_stats);
    Ok(StreamedHistory {
        inspection,
        schema_observation,
        capability_snapshot,
        typed_evidence,
        native_entities,
        warnings,
    })
}

/// Converts Safari's Cocoa timestamp format to Unix milliseconds.
pub fn safari_time_to_unix_ms(value: f64) -> i64 {
    (((value + SAFARI_UNIX_EPOCH_OFFSET_SECONDS) * 1_000.0).round() as i64).max(0)
}

/// Converts Unix milliseconds back into Safari's Cocoa timestamp format.
pub fn unix_ms_to_safari_time(value: i64) -> f64 {
    (value.max(0) as f64 / 1_000.0) - SAFARI_UNIX_EPOCH_OFFSET_SECONDS
}

/// Converts Safari's Cocoa timestamp format to RFC3339.
pub fn safari_time_to_iso(value: f64) -> String {
    let milliseconds = safari_time_to_unix_ms(value);
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

fn validate_required_tables(inspection: &DatabaseInspection) -> Result<(), ParseError> {
    for table_name in ["history_items", "history_visits"] {
        if !inspection.table_names.iter().any(|existing| existing == table_name) {
            return Err(ParseError::MissingTable { table: table_name });
        }
    }
    Ok(())
}

fn parsed_url_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedUrl> {
    let last_visit_time = row.get::<_, Option<f64>>(4)?.unwrap_or_default();
    Ok(ParsedUrl {
        source_url_id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
        typed_count: 0,
        last_visit_ms: safari_time_to_unix_ms(last_visit_time),
        last_visit_iso: safari_time_to_iso(last_visit_time),
        hidden: false,
    })
}

fn parsed_visit_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedVisit> {
    let visit_time = row.get::<_, f64>(4)?;
    Ok(ParsedVisit {
        source_visit_id: row.get(0)?,
        source_url_id: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        visit_time_ms: safari_time_to_unix_ms(visit_time),
        visit_time_iso: safari_time_to_iso(visit_time),
        from_visit: None,
        transition: None,
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: Some("safari".to_string()),
    })
}

fn safari_visit_extra_columns(
    schema_observation: &SchemaObservation,
) -> Vec<SafariVisitExtraColumn> {
    let has_visit_column = |column_name: &str| {
        schema_observation.tables.iter().any(|table| {
            table.name == "history_visits"
                && table.present
                && table.columns.iter().any(|column| column.name == column_name)
        })
    };
    [
        SafariVisitExtraColumn::LoadSuccessful,
        SafariVisitExtraColumn::HttpNonGet,
        SafariVisitExtraColumn::Synthesized,
        SafariVisitExtraColumn::RedirectSource,
        SafariVisitExtraColumn::RedirectDestination,
        SafariVisitExtraColumn::Origin,
        SafariVisitExtraColumn::Generation,
        SafariVisitExtraColumn::Attributes,
        SafariVisitExtraColumn::Score,
    ]
    .into_iter()
    .filter(|column| has_visit_column(column.column_name()))
    .collect()
}

fn safari_visits_sql(extra_columns: &[SafariVisitExtraColumn]) -> String {
    let mut select_list = vec![
        "history_visits.id".to_string(),
        "history_visits.history_item".to_string(),
        "history_items.url".to_string(),
        "history_visits.title".to_string(),
        "history_visits.visit_time".to_string(),
    ];
    select_list.extend(
        extra_columns.iter().map(|column| format!("history_visits.{}", column.column_name())),
    );
    format!(
        "SELECT {}\nFROM history_visits\nJOIN history_items\n  ON history_items.id = history_visits.history_item\nWHERE history_visits.id > ?1\nORDER BY history_visits.id ASC",
        select_list.join(",\n  ")
    )
}

fn extend_typed_evidence_from_visit_row(
    row: &Row<'_>,
    visit: &ParsedVisit,
    extra_columns: &[SafariVisitExtraColumn],
    typed_evidence: &mut TypedEvidenceBatch,
    stats: &mut SafariVisitEvidenceStats,
) -> rusqlite::Result<()> {
    for (offset, column) in extra_columns.iter().enumerate() {
        let index = 5 + offset;
        match column {
            SafariVisitExtraColumn::RedirectSource => {
                if let Some(value) = row.get::<_, Option<i64>>(index)? {
                    typed_evidence.navigation.push(NavigationEvidence {
                        source_visit_id: visit.source_visit_id,
                        edge_kind: "safari.redirect_source".to_string(),
                        target_visit_id: Some(value),
                        target_url: None,
                        transition: None,
                        source_field: column.source_field(),
                    });
                    stats.redirect_edges += 1;
                }
            }
            SafariVisitExtraColumn::RedirectDestination => {
                if let Some(value) = row.get::<_, Option<i64>>(index)? {
                    typed_evidence.navigation.push(NavigationEvidence {
                        source_visit_id: visit.source_visit_id,
                        edge_kind: "safari.redirect_destination".to_string(),
                        target_visit_id: Some(value),
                        target_url: None,
                        transition: None,
                        source_field: column.source_field(),
                    });
                    stats.redirect_edges += 1;
                }
            }
            SafariVisitExtraColumn::Score => {
                if let Some(value) = row.get::<_, Option<f64>>(index)? {
                    typed_evidence.engagement.push(EngagementEvidence {
                        source_visit_id: visit.source_visit_id,
                        metric_key: "safari.score".to_string(),
                        metric_value_int: None,
                        metric_value_real: Some(value),
                        source_field: column.source_field(),
                    });
                    stats.scores += 1;
                }
            }
            SafariVisitExtraColumn::LoadSuccessful
            | SafariVisitExtraColumn::HttpNonGet
            | SafariVisitExtraColumn::Synthesized
            | SafariVisitExtraColumn::Origin
            | SafariVisitExtraColumn::Generation
            | SafariVisitExtraColumn::Attributes => {
                if let Some(value) = row.get::<_, Option<i64>>(index)? {
                    let context_key = match column {
                        SafariVisitExtraColumn::LoadSuccessful => "safari.load_successful",
                        SafariVisitExtraColumn::HttpNonGet => "safari.http_non_get",
                        SafariVisitExtraColumn::Synthesized => "safari.synthesized",
                        SafariVisitExtraColumn::Origin => "safari.origin",
                        SafariVisitExtraColumn::Generation => "safari.generation",
                        SafariVisitExtraColumn::Attributes => "safari.attributes",
                        SafariVisitExtraColumn::RedirectSource
                        | SafariVisitExtraColumn::RedirectDestination
                        | SafariVisitExtraColumn::Score => unreachable!(),
                    };
                    let value_json = match column {
                        SafariVisitExtraColumn::LoadSuccessful
                        | SafariVisitExtraColumn::HttpNonGet
                        | SafariVisitExtraColumn::Synthesized => json!(value != 0).to_string(),
                        SafariVisitExtraColumn::Origin
                        | SafariVisitExtraColumn::Generation
                        | SafariVisitExtraColumn::Attributes => json!(value).to_string(),
                        SafariVisitExtraColumn::RedirectSource
                        | SafariVisitExtraColumn::RedirectDestination
                        | SafariVisitExtraColumn::Score => unreachable!(),
                    };
                    typed_evidence.context.push(ContextEvidence {
                        source_visit_id: Some(visit.source_visit_id),
                        source_url_id: Some(visit.source_url_id),
                        context_key: context_key.to_string(),
                        value_json,
                        source_field: column.source_field(),
                    });
                    match column {
                        SafariVisitExtraColumn::LoadSuccessful => stats.load_outcomes += 1,
                        SafariVisitExtraColumn::HttpNonGet => stats.http_methods += 1,
                        SafariVisitExtraColumn::Synthesized => stats.synthesized_rows += 1,
                        SafariVisitExtraColumn::Origin
                        | SafariVisitExtraColumn::Generation
                        | SafariVisitExtraColumn::Attributes => {}
                        SafariVisitExtraColumn::RedirectSource
                        | SafariVisitExtraColumn::RedirectDestination
                        | SafariVisitExtraColumn::Score => unreachable!(),
                    }
                }
            }
        }
    }
    Ok(())
}

fn build_capability_snapshot(
    visit_count: usize,
    evidence_stats: &SafariVisitEvidenceStats,
) -> crate::types::CapabilitySnapshot {
    capability_snapshot(vec![
        CapabilityCoverage {
            key: "canonical.history_visits".to_string(),
            available: visit_count > 0,
            populated_rows: visit_count,
            total_rows: visit_count,
            notes: vec!["Safari History.db visits".to_string()],
        },
        CapabilityCoverage {
            key: "safari.redirect_edges".to_string(),
            available: evidence_stats.redirect_edges > 0,
            populated_rows: evidence_stats.redirect_edges,
            total_rows: visit_count,
            notes: vec!["history_visits.redirect_source and redirect_destination".to_string()],
        },
        CapabilityCoverage {
            key: "safari.load_outcome".to_string(),
            available: evidence_stats.load_outcomes > 0,
            populated_rows: evidence_stats.load_outcomes,
            total_rows: visit_count,
            notes: vec!["history_visits.load_successful".to_string()],
        },
        CapabilityCoverage {
            key: "safari.http_method".to_string(),
            available: evidence_stats.http_methods > 0,
            populated_rows: evidence_stats.http_methods,
            total_rows: visit_count,
            notes: vec!["history_visits.http_non_get".to_string()],
        },
        CapabilityCoverage {
            key: "safari.synthesized".to_string(),
            available: evidence_stats.synthesized_rows > 0,
            populated_rows: evidence_stats.synthesized_rows,
            total_rows: visit_count,
            notes: vec!["history_visits.synthesized".to_string()],
        },
        CapabilityCoverage {
            key: "safari.score".to_string(),
            available: evidence_stats.scores > 0,
            populated_rows: evidence_stats.scores,
            total_rows: visit_count,
            notes: vec!["history_visits.score".to_string()],
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::convert::Infallible;
    use tempfile::tempdir;

    #[derive(Default)]
    struct NonRetainingEvidenceSink {
        urls: usize,
        visits: usize,
        source_evidence_chunks: usize,
        navigation_evidence: usize,
        engagement_evidence: usize,
        context_evidence: usize,
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

        fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<(), Self::Error> {
            self.source_evidence_chunks += 1;
            self.navigation_evidence += chunk.typed_evidence.navigation.len();
            self.engagement_evidence += chunk.typed_evidence.engagement.len();
            self.context_evidence += chunk.typed_evidence.context.len();
            self.native_entities += chunk.native_entities.len();
            Ok(())
        }

        fn retain_source_evidence_in_report(&self) -> bool {
            false
        }
    }

    fn write_history_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open safari fixture");
        connection
            .execute_batch(
                "CREATE TABLE history_items (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL
                 );
                 CREATE TABLE history_visits (
                   id INTEGER PRIMARY KEY,
                   history_item INTEGER NOT NULL,
                   title TEXT,
                   visit_time REAL NOT NULL
                 );",
            )
            .expect("create safari schema");
        connection
            .execute(
                "INSERT INTO history_items (id, url) VALUES (?1, ?2)",
                params![5_i64, "https://example.com/safari"],
            )
            .expect("insert history item");
        connection
            .execute(
                "INSERT INTO history_visits (id, history_item, title, visit_time)
                 VALUES (?1, ?2, ?3, ?4)",
                params![9_i64, 5_i64, "Safari Example", 765_838_800.0_f64],
            )
            .expect("insert safari visit");
    }

    fn write_current_schema_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open current safari fixture");
        connection
            .execute_batch(
                "CREATE TABLE history_items (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL
                 );
                 CREATE TABLE history_visits (
                   id INTEGER PRIMARY KEY,
                   history_item INTEGER NOT NULL,
                   title TEXT,
                   visit_time REAL NOT NULL,
                   load_successful INTEGER,
                   http_non_get INTEGER,
                   synthesized INTEGER,
                   redirect_source INTEGER,
                   redirect_destination INTEGER,
                   origin INTEGER,
                   generation INTEGER,
                   attributes INTEGER,
                   score REAL
                 );",
            )
            .expect("create current safari schema");
        connection
            .execute(
                "INSERT INTO history_items (id, url) VALUES (?1, ?2)",
                params![5_i64, "https://example.com/safari"],
            )
            .expect("insert current history item");
        connection
            .execute(
                "INSERT INTO history_visits (
                   id, history_item, title, visit_time, load_successful,
                   http_non_get, synthesized, redirect_source, redirect_destination,
                   origin, generation, attributes, score
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    9_i64,
                    5_i64,
                    "Safari Start",
                    765_838_800.0_f64,
                    1_i64,
                    0_i64,
                    0_i64,
                    Option::<i64>::None,
                    Some(10_i64),
                    1_i64,
                    2_i64,
                    4_i64,
                    0.75_f64,
                ],
            )
            .expect("insert current safari first visit");
        connection
            .execute(
                "INSERT INTO history_visits (
                   id, history_item, title, visit_time, load_successful,
                   http_non_get, synthesized, redirect_source, redirect_destination,
                   origin, generation, attributes, score
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    10_i64,
                    5_i64,
                    "Safari Finish",
                    765_838_801.0_f64,
                    0_i64,
                    1_i64,
                    1_i64,
                    Some(9_i64),
                    Option::<i64>::None,
                    1_i64,
                    3_i64,
                    8_i64,
                    0.25_f64,
                ],
            )
            .expect("insert current safari second visit");
    }

    #[test]
    fn parse_history_reads_safari_items_and_visits() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        write_history_fixture(&history_path);

        let parsed = parse_history(&history_path, 0, 0).expect("parse safari history");
        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
        assert_eq!(parsed.urls[0].title.as_deref(), Some("Safari Example"));
        assert_eq!(parsed.visits[0].source_visit_id, 9);
        assert_eq!(parsed.visits[0].app_id.as_deref(), Some("safari"));
        assert!(parsed.warnings.iter().any(|warning| warning.code == "baseline-support"));
    }

    #[test]
    fn parse_history_preserves_current_safari_visit_metadata() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        write_current_schema_fixture(&history_path);

        let parsed = parse_history(&history_path, 0, 0).expect("parse current safari history");

        assert_eq!(parsed.visits.len(), 2);
        assert_eq!(parsed.typed_evidence.navigation.len(), 2);
        assert_eq!(parsed.typed_evidence.engagement.len(), 2);
        assert!(parsed.typed_evidence.context.iter().any(|item| {
            item.context_key == "safari.load_successful" && item.value_json == "false"
        }));
        assert!(parsed.native_entities.iter().any(|entity| {
            entity.entity_kind == "safari-history-visit-row"
                && entity.payload_json.contains("load_successful")
                && entity.payload_json.contains("redirect_destination")
        }));
        assert!(parsed.capability_snapshot.items.iter().any(|item| {
            item.key == "safari.redirect_edges" && item.available && item.populated_rows == 2
        }));
    }

    #[test]
    fn stream_history_can_move_source_evidence_out_of_the_returned_report() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        write_current_schema_fixture(&history_path);
        let mut sink = NonRetainingEvidenceSink::default();

        let streamed = stream_history(&history_path, 0, 0, 1, &mut sink).expect("stream safari");

        assert_eq!(sink.urls, 1);
        assert_eq!(sink.visits, 2);
        assert!(sink.source_evidence_chunks >= 3);
        assert_eq!(sink.navigation_evidence, 2);
        assert_eq!(sink.engagement_evidence, 2);
        assert!(sink.context_evidence >= 6);
        assert!(sink.native_entities >= 3);
        assert!(streamed.native_entities.is_empty());
        assert!(streamed.typed_evidence.navigation.is_empty());
        assert!(streamed.typed_evidence.engagement.is_empty());
        assert!(streamed.typed_evidence.context.is_empty());
        assert!(
            streamed
                .capability_snapshot
                .items
                .iter()
                .any(|item| { item.key == "safari.redirect_edges" && item.populated_rows == 2 })
        );
    }

    #[test]
    fn parse_history_reads_reference_safari_database_shape() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../reference/browserexport/tests/databases/safari.sqlite");

        let parsed = parse_history(&fixture_path, 0, 0).expect("parse reference safari fixture");

        assert_eq!(parsed.urls.len(), 2);
        assert_eq!(parsed.visits.len(), 3);
        assert_eq!(parsed.typed_evidence.navigation.len(), 2);
        assert_eq!(
            parsed
                .typed_evidence
                .context
                .iter()
                .filter(|item| item.context_key == "safari.load_successful")
                .count(),
            3
        );
        assert!(parsed.native_entities.iter().any(|entity| {
            entity.entity_kind == "safari-history-visit-row"
                && entity.payload_json.contains("load_successful")
                && entity.payload_json.contains("score")
        }));
    }

    #[test]
    fn inspect_history_reports_missing_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute("CREATE TABLE history_items (id INTEGER PRIMARY KEY)", [])
            .expect("create history items table");

        let inspection = inspect_history(&history_path).expect("inspect safari history");
        assert!(inspection.warnings.iter().any(|warning| {
            warning.message.contains("required Safari table `history_visits` is missing")
        }));
    }

    #[test]
    fn parse_history_requires_safari_required_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "CREATE TABLE history_items (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL
                 )",
                [],
            )
            .expect("create history items table");

        let error = parse_history(&history_path, 0, 0).expect_err("missing visits should fail");
        assert!(matches!(error, ParseError::MissingTable { table: "history_visits" }));
    }

    #[test]
    fn safari_time_helpers_keep_dates_stable() {
        assert_eq!(safari_time_to_unix_ms(0.0), 978_307_200_000);
        assert_eq!(unix_ms_to_safari_time(978_307_200_000), 0.0);
        assert_eq!(
            safari_time_to_unix_ms(unix_ms_to_safari_time(1_744_146_000_000)),
            1_744_146_000_000
        );
        assert_eq!(
            safari_time_to_iso(-SAFARI_UNIX_EPOCH_OFFSET_SECONDS),
            "1970-01-01T00:00:00+00:00"
        );
        assert!(safari_time_to_iso(765_838_800.0).starts_with("2025-04-"));
    }

    #[test]
    fn parse_history_respects_visit_and_url_cursors() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        write_history_fixture(&history_path);

        let parsed = parse_history(&history_path, 9, 1_744_146_000_000).expect("cursor parse");

        assert_eq!(parsed.urls.len(), 1);
        assert!(parsed.visits.is_empty());
    }
}
