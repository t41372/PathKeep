//! Parser and watermark helpers for canonical backup ingest.
//!
//! ## Responsibilities
//! - Parse staged browser snapshots using the correct parser for each browser family.
//! - Track per-profile incremental watermarks so repeat backups stay bounded.
//! - Decide when a new raw source checkpoint is required.
//!
//! ## Not responsible for
//! - Writing canonical archive rows or source-evidence rows.
//! - Running archive manifests, run ledgers, or retention workflows.
//! - Picking which profiles participate in a backup run.
//!
//! ## Dependencies
//! - `browser_history_parser` family-specific parsers and cursors.
//! - Watermark storage tables owned by the parent archive schema.
//!
//! ## Performance notes
//! - Parser output is still materialized in memory today, so watermark filtering
//!   is the main guardrail that keeps repeat backups from replaying old rows.

use super::super::*;
use browser_history_parser::{
    ChromiumReadCursor, HistoryDatabaseSet, ParsedHistory, chromium, firefox, safari,
};
use chrono::{DateTime, Duration, Utc};

/// Tracks the last successfully ingested source cursors for one profile.
#[derive(Debug, Default)]
pub(super) struct Watermark {
    pub(super) last_visit_id: i64,
    pub(super) last_url_last_visit_time: i64,
    pub(super) last_download_id: i64,
    pub(super) last_favicon_last_updated: i64,
    pub(super) last_checkpoint_at: Option<String>,
    pub(super) last_schema_hash: Option<String>,
    pub(super) last_source_batch_id: Option<i64>,
    pub(super) updated_at: String,
}

/// Carries parser output plus the newest cursors observed in one staged snapshot.
#[derive(Debug)]
pub(super) struct ParsedProfileSnapshot {
    pub(super) history: ParsedHistory,
    pub(super) last_visit_id: i64,
    pub(super) last_url_marker: Option<i64>,
    pub(super) last_download_id: Option<i64>,
    pub(super) last_favicon_marker: Option<i64>,
}

/// Parses a saved source checkpoint without incremental cursors so restore preview can size the replay.
pub(super) fn preview_snapshot_counts(
    snapshot: &ProfileSnapshot,
    config: &AppConfig,
) -> Result<(usize, usize, usize)> {
    let parsed = parse_profile_snapshot(snapshot, config, &Watermark::default())?;
    Ok((parsed.history.visits.len(), parsed.history.urls.len(), parsed.history.downloads.len()))
}

/// Parses one staged profile snapshot using the correct browser-family parser and cursors.
pub(super) fn parse_profile_snapshot(
    snapshot: &ProfileSnapshot,
    config: &AppConfig,
    watermark: &Watermark,
) -> Result<ParsedProfileSnapshot> {
    match snapshot.profile.browser_family.as_str() {
        "chromium" => {
            let history = chromium::parse_history(
                &HistoryDatabaseSet {
                    history_path: snapshot.history_path.clone(),
                    favicons_path: if config.capture_favicons {
                        snapshot.favicons_path.clone()
                    } else {
                        None
                    },
                },
                ChromiumReadCursor {
                    after_visit_id: watermark.last_visit_id,
                    after_url_last_visit_time: watermark.last_url_last_visit_time,
                    after_download_id: watermark.last_download_id,
                    after_favicon_last_updated: watermark.last_favicon_last_updated,
                },
            )?;
            let last_visit_id =
                history.visits.iter().map(|visit| visit.source_visit_id).max().unwrap_or_default();
            let last_url_marker =
                history.urls.iter().map(|url| ms_to_chromium_time(url.last_visit_ms)).max();
            let last_download_id =
                history.downloads.iter().map(|download| download.source_download_id).max();
            let last_favicon_marker = history
                .favicons
                .iter()
                .map(|favicon| ms_to_chromium_time(favicon.last_updated_ms))
                .max();

            Ok(ParsedProfileSnapshot {
                history,
                last_visit_id,
                last_url_marker,
                last_download_id,
                last_favicon_marker,
            })
        }
        "firefox" => {
            let history = firefox::parse_history(
                &snapshot.history_path,
                watermark.last_visit_id,
                watermark.last_url_last_visit_time,
            )?;
            let last_visit_id =
                history.visits.iter().map(|visit| visit.source_visit_id).max().unwrap_or_default();
            let last_url_marker = history.urls.iter().map(|url| url.last_visit_ms).max();
            Ok(ParsedProfileSnapshot {
                history,
                last_visit_id,
                last_url_marker,
                last_download_id: None,
                last_favicon_marker: None,
            })
        }
        "safari" => {
            let history = safari::parse_history(
                &snapshot.history_path,
                watermark.last_visit_id,
                watermark.last_url_last_visit_time,
            )?;
            let last_visit_id =
                history.visits.iter().map(|visit| visit.source_visit_id).max().unwrap_or_default();
            let last_url_marker = history.urls.iter().map(|url| url.last_visit_ms).max();
            Ok(ParsedProfileSnapshot {
                history,
                last_visit_id,
                last_url_marker,
                last_download_id: None,
                last_favicon_marker: None,
            })
        }
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
}

/// Loads the last successful incremental cursors for one source profile.
pub(super) fn load_watermark(archive: &Transaction<'_>, profile_id: &str) -> Result<Watermark> {
    archive
        .query_row(
            "SELECT
               last_visit_id,
               last_url_last_visit_time,
               last_download_id,
               last_favicon_last_updated,
               last_checkpoint_at,
               last_schema_hash,
               last_source_batch_id,
               updated_at
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
                })
            },
        )
        .optional()
        .map(|value| {
            value.unwrap_or_else(|| Watermark { updated_at: now_rfc3339(), ..Watermark::default() })
        })
        .map_err(Into::into)
}

/// Saves the next incremental cursors after one staged profile has been processed.
pub(super) fn save_watermark(
    archive: &Transaction<'_>,
    profile_id: &str,
    watermark: &Watermark,
) -> Result<()> {
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
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(profile_id) DO UPDATE SET
           last_visit_id = excluded.last_visit_id,
           last_url_last_visit_time = excluded.last_url_last_visit_time,
           last_download_id = excluded.last_download_id,
           last_favicon_last_updated = excluded.last_favicon_last_updated,
           last_checkpoint_at = excluded.last_checkpoint_at,
           last_schema_hash = excluded.last_schema_hash,
           last_source_batch_id = excluded.last_source_batch_id,
           updated_at = excluded.updated_at",
        params![
            profile_id,
            watermark.last_visit_id,
            watermark.last_url_last_visit_time,
            watermark.last_download_id,
            watermark.last_favicon_last_updated,
            watermark.last_checkpoint_at,
            watermark.last_schema_hash,
            watermark.last_source_batch_id,
            watermark.updated_at,
        ],
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

/// Converts canonical visit milliseconds back into Chromium's microsecond epoch for fingerprints.
fn ms_to_chromium_time(value_ms: i64) -> i64 {
    unix_micros_to_chrome_time(value_ms.saturating_mul(1_000))
}
