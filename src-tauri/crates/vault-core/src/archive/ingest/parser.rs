//! Responsibilities: persist native-unit cursors and inspect staged source counts.
//! Not responsible for: canonical row writes, database ownership or run completion.
//! Failure modes: absent/regressing cursors, lost checkpoint/evidence provenance,
//! truncated native timestamps, unsupported or malformed source files.
//! Tests: watermark_helpers and the real parser-to-Store dedup suites.

use super::super::*;
use browser_history_parser::native_visits::VisitMark;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags};

/// Tracks the last successfully ingested source cursors for one profile.
#[derive(Debug, Clone, Default)]
pub(crate) struct Watermark {
    pub(crate) last_visit_id: i64,
    /// The visit row `last_visit_id` held when it was read: its browser time, address and kind. A row
    /// id names another visit once the browser numbers its rows again; the next backup reads
    /// only the rows after it while that row still holds this visit.
    pub(crate) last_visit_key: Option<String>,
    pub(crate) last_visit_url: Option<String>,
    pub(crate) last_visit_kind: Option<i64>,
    /// When a backup last read every visit in the browser file.
    pub(crate) last_full_scan_at: Option<String>,
    /// When the read of every row now under way started; `None` once it finished.
    pub(crate) full_scan_started_at: Option<String>,
    /// The newest visit time (Unix ms) any backup read since the last read of every row started.
    pub(crate) newest_visit_ms: i64,
    /// The visit row with the lowest id when a backup last read the file, and the visit it held
    /// (browser time, address and kind). A table made again starts its ids over.
    pub(crate) first_visit_id: Option<i64>,
    pub(crate) first_visit_key: Option<String>,
    pub(crate) first_visit_url: Option<String>,
    pub(crate) first_visit_kind: Option<i64>,
    pub(crate) last_url_last_visit_time: i64,
    pub(crate) last_search_url_time: i64,
    pub(crate) url_scan: Option<browser_history_parser::UrlScan>,
    pub(crate) last_download_id: i64,
    pub(crate) last_favicon_last_updated: i64,
    pub(crate) last_checkpoint_at: Option<String>,
    pub(crate) last_schema_hash: Option<String>,
    pub(crate) last_source_batch_id: Option<i64>,
    pub(crate) updated_at: String,
}

impl Watermark {
    /// Records the row with the lowest id this read found in the file.
    pub(crate) fn set_first_visit(&mut self, first: &(i64, VisitMark)) {
        self.first_visit_id = Some(first.0);
        self.first_visit_key = Some(first.1.key.clone());
        self.first_visit_url = Some(first.1.url.clone());
        self.first_visit_kind = first.1.kind;
    }

    /// Takes `other`'s visit mark: the newest row a read reached, the visit it holds, and the
    /// newest visit time read (`consumer::track_newest`).
    pub(crate) fn take_visit_mark(&mut self, other: &Watermark) {
        self.last_visit_id = other.last_visit_id;
        self.last_visit_key.clone_from(&other.last_visit_key);
        self.last_visit_url.clone_from(&other.last_visit_url);
        self.last_visit_kind = other.last_visit_kind;
        self.newest_visit_ms = other.newest_visit_ms;
    }
}

/// Parses a saved source checkpoint without incremental cursors so restore preview can size the replay.
pub(super) fn preview_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    match snapshot.profile.browser_family.as_str() {
        "chromium" => preview_chromium_snapshot_counts(snapshot),
        "firefox" => preview_firefox_snapshot_counts(snapshot),
        "safari" => preview_safari_snapshot_counts(snapshot),
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
}

/// Loads the last successful incremental cursors for one source profile.
pub(super) fn load_watermark(
    archive: &vault_store::Session<'_>,
    profile_id: &str,
) -> vault_store::Result<Watermark> {
    archive
        .query(
            "SELECT
               last_visit_id,
               last_url_last_visit_time,
               last_download_id,
               last_favicon_last_updated,
               last_checkpoint_at,
               last_schema_hash,
               last_source_batch_id,
               updated_at, last_search_url_time, url_scan_json,
               last_visit_key, last_visit_url, last_full_scan_at, full_scan_started_at,
               newest_visit_ms, first_visit_id, first_visit_key, first_visit_url,
               last_visit_kind, first_visit_kind
             FROM profile_watermarks
             WHERE profile_id = ?1",
            [profile_id],
            |row| {
                Ok(Watermark {
                    last_visit_id: row.get(0)?,
                    last_url_last_visit_time: row.get(1)?,
                    last_download_id: row.get(2)?,
                    last_favicon_last_updated: row.get(3)?,
                    last_checkpoint_at: row.get(4)?,
                    last_schema_hash: row.get(5)?,
                    last_source_batch_id: row.get(6)?,
                    updated_at: row.get(7)?,
                    last_search_url_time: row.get(8)?,
                    url_scan: row
                        .get::<Option<String>>(9)?
                        .map(|text| serde_json::from_str(&text))
                        .transpose()
                        .map_err(|error| vault_store::Error::Operation(Box::new(error)))?,
                    last_visit_key: row.get(10)?,
                    last_visit_url: row.get(11)?,
                    last_full_scan_at: row.get(12)?,
                    full_scan_started_at: row.get(13)?,
                    newest_visit_ms: row.get(14)?,
                    first_visit_id: row.get(15)?,
                    first_visit_key: row.get(16)?,
                    first_visit_url: row.get(17)?,
                    last_visit_kind: row.get(18)?,
                    first_visit_kind: row.get(19)?,
                })
            },
        )
        .map(|mut value| {
            value
                .pop()
                .unwrap_or_else(|| Watermark { updated_at: now_rfc3339(), ..Watermark::default() })
        })
}

/// Saves the next incremental cursors after one staged profile has been processed. The last
/// row read and the visit it held move together, only forward.
pub(super) fn save_watermark(
    archive: &vault_store::Session<'_>,
    profile_id: &str,
    watermark: &Watermark,
) -> vault_store::Result<()> {
    archive.execute(
        "INSERT INTO profile_watermarks (
           profile_id,
           last_visit_id,
           last_url_last_visit_time,
           last_download_id,
           last_favicon_last_updated,
           last_checkpoint_at,
           last_schema_hash,
           last_source_batch_id,
           updated_at, last_search_url_time, url_scan_json,
           last_visit_key, last_visit_url, last_full_scan_at, full_scan_started_at,
           newest_visit_ms, first_visit_id, first_visit_key, first_visit_url,
           last_visit_kind, first_visit_kind
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
           ?19, ?20, ?21)
         ON CONFLICT(profile_id) DO UPDATE SET
           last_visit_key = CASE WHEN excluded.last_visit_id >= profile_watermarks.last_visit_id
             THEN excluded.last_visit_key ELSE profile_watermarks.last_visit_key END,
           last_visit_url = CASE WHEN excluded.last_visit_id >= profile_watermarks.last_visit_id
             THEN excluded.last_visit_url ELSE profile_watermarks.last_visit_url END,
           last_visit_kind = CASE WHEN excluded.last_visit_id >= profile_watermarks.last_visit_id
             THEN excluded.last_visit_kind ELSE profile_watermarks.last_visit_kind END,
           last_full_scan_at = COALESCE(excluded.last_full_scan_at, profile_watermarks.last_full_scan_at),
           full_scan_started_at = COALESCE(excluded.full_scan_started_at, profile_watermarks.full_scan_started_at),
           newest_visit_ms = MAX(profile_watermarks.newest_visit_ms, excluded.newest_visit_ms),
           last_visit_id = MAX(profile_watermarks.last_visit_id, excluded.last_visit_id),
           last_url_last_visit_time = MAX(profile_watermarks.last_url_last_visit_time, excluded.last_url_last_visit_time),
           last_download_id = MAX(profile_watermarks.last_download_id, excluded.last_download_id),
           last_favicon_last_updated = MAX(profile_watermarks.last_favicon_last_updated, excluded.last_favicon_last_updated),
           last_checkpoint_at = excluded.last_checkpoint_at,
           last_schema_hash = excluded.last_schema_hash,
           last_source_batch_id = excluded.last_source_batch_id,
           last_search_url_time = MAX(profile_watermarks.last_search_url_time, excluded.last_search_url_time),
           url_scan_json = excluded.url_scan_json,
           first_visit_id = COALESCE(excluded.first_visit_id, profile_watermarks.first_visit_id),
           first_visit_key = CASE WHEN excluded.first_visit_id IS NULL
             THEN profile_watermarks.first_visit_key ELSE excluded.first_visit_key END,
           first_visit_url = CASE WHEN excluded.first_visit_id IS NULL
             THEN profile_watermarks.first_visit_url ELSE excluded.first_visit_url END,
           first_visit_kind = CASE WHEN excluded.first_visit_id IS NULL
             THEN profile_watermarks.first_visit_kind ELSE excluded.first_visit_kind END,
           updated_at = excluded.updated_at",
        vault_store::params![
            profile_id,
            watermark.last_visit_id,
            watermark.last_url_last_visit_time,
            watermark.last_download_id,
            watermark.last_favicon_last_updated,
            watermark.last_checkpoint_at.clone(),
            watermark.last_schema_hash.clone(),
            watermark.last_source_batch_id,
            watermark.updated_at.clone(),
            watermark.last_search_url_time,
            watermark.url_scan.map(|scan|serde_json::to_string(&scan).expect("scalar URL scan")),
            watermark.last_visit_key.clone(),
            watermark.last_visit_url.clone(),
            watermark.last_full_scan_at.clone(),
            watermark.full_scan_started_at.clone(),
            watermark.newest_visit_ms,
            watermark.first_visit_id,
            watermark.first_visit_key.clone(),
            watermark.first_visit_url.clone(),
            watermark.last_visit_kind,
            watermark.first_visit_kind,
        ],
    )?;
    Ok(())
}

/// Ends a read of every row: it is the latest full read, and no longer under way. The only
/// write that clears the mark: a checkpoint replay, which saves no cursors, leaves it alone.
pub(super) fn finish_full_read(
    archive: &vault_store::Session<'_>,
    profile_id: &str,
    finished_at: &str,
) -> vault_store::Result<()> {
    archive.execute(
        "UPDATE profile_watermarks SET last_full_scan_at=?2, full_scan_started_at=NULL WHERE profile_id=?1",
        [profile_id, finished_at],
    )?;
    Ok(())
}

/// Starts a read of every row: the cursors go back to the start, and the read is marked under
/// way. `save_watermark` only moves cursors forward, and after the browser numbered its rows
/// again the new ones are lower. Rows are read in id order, so a read stopped part way leaves a
/// cursor the next backup carries on from while the mark is set.
pub(super) fn begin_full_read(
    archive: &vault_store::Session<'_>,
    profile_id: &str,
    started_at: &str,
) -> vault_store::Result<()> {
    archive.execute(
        "INSERT INTO profile_watermarks(profile_id,updated_at,full_scan_started_at) VALUES(?1,?2,?2)
         ON CONFLICT(profile_id) DO UPDATE SET last_visit_id=0, last_visit_key=NULL,
           last_visit_url=NULL, last_visit_kind=NULL, last_url_last_visit_time=0, last_search_url_time=0,
           url_scan_json=NULL, newest_visit_ms=0, full_scan_started_at=excluded.full_scan_started_at",
        [profile_id, started_at],
    )?;
    Ok(())
}

/// Decides whether this profile should emit a raw source checkpoint during the current backup.
pub(super) fn should_checkpoint(
    watermark: &Watermark,
    schema_hash: &str,
    checkpoint_days: u64,
) -> bool {
    if watermark.last_schema_hash.as_deref() != Some(schema_hash) {
        return true;
    }
    let Some(last_checkpoint_at) = &watermark.last_checkpoint_at else {
        return true;
    };
    let Ok(last_checkpoint_at) = DateTime::parse_from_rfc3339(last_checkpoint_at) else {
        return true;
    };
    Utc::now() - last_checkpoint_at.with_timezone(&Utc) > Duration::days(checkpoint_days as i64)
}

fn preview_chromium_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    let connection = open_snapshot_connection(&snapshot.history_path)?;
    let urls = query_table_count(&connection, "urls", "SELECT COUNT(*) FROM urls")?;
    let visits = query_table_count(&connection, "visits", "SELECT COUNT(*) FROM visits")?;
    let downloads =
        query_optional_table_count(&connection, "downloads", "SELECT COUNT(*) FROM downloads")?;
    Ok((visits, urls, downloads))
}

fn preview_firefox_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    let connection = open_snapshot_connection(&snapshot.history_path)?;
    let urls = query_table_count(
        &connection,
        "moz_places",
        "SELECT COUNT(*) FROM moz_places WHERE last_visit_date IS NOT NULL",
    )?;
    let visits = query_table_count(
        &connection,
        "moz_historyvisits",
        "SELECT COUNT(*) FROM moz_historyvisits",
    )?;
    Ok((visits, urls, 0))
}

fn preview_safari_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    let connection = open_snapshot_connection(&snapshot.history_path)?;
    let visits =
        query_table_count(&connection, "history_visits", "SELECT COUNT(*) FROM history_visits")?;
    let urls = query_table_count(
        &connection,
        "history_items",
        "SELECT COUNT(DISTINCT history_item) FROM history_visits",
    )?;
    Ok((visits, urls, 0))
}

fn open_snapshot_connection(path: &std::path::Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening checkpoint {}", path.display()))
}

fn query_table_count(connection: &Connection, table: &str, sql: &str) -> Result<usize> {
    if !table_exists(connection, table)? {
        anyhow::bail!("checkpoint is missing required table `{table}`");
    }
    query_count(connection, sql)
}

fn query_optional_table_count(connection: &Connection, table: &str, sql: &str) -> Result<usize> {
    if !table_exists(connection, table)? {
        return Ok(0);
    }
    query_count(connection, sql)
}

fn query_count(connection: &Connection, sql: &str) -> Result<usize> {
    connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .map(|count| count as usize)
        .map_err(Into::into)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM sqlite_master
               WHERE type = 'table' AND name = ?1
             )",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|exists| exists != 0)
        .map_err(Into::into)
}
