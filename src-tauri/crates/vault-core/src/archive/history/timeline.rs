//! Responsibilities: keyset timeline pages from committed canonical visits.
//! Not responsible for: search recall, storage ownership or export rendering.
//! Failure modes: ties skip/repeat visits, hidden rows leak, reads wait on writes,
//! filtered pages disappear, render queries scan facts. Tests: timeline_tests.
use super::pagination::{HistoryCursor, page_count, parse_history_cursor};
use crate::{HistoryEntry, HistoryQuery, HistoryQueryResponse, Vault, utils::url_domain};
use anyhow::{Context, Result};
use vault_store::{Db, Session, Value, params};

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;

/// Reads one timeline page; search requests are dispatched by the worker's recall entrypoint.
pub fn list_history(vault: &Vault, query: HistoryQuery) -> Result<HistoryQueryResponse> {
    anyhow::ensure!(
        query.q.as_ref().is_none_or(|q| q.trim().is_empty()),
        "search requests must use the search dispatcher"
    );
    anyhow::ensure!(
        query.page.is_none_or(|page| page <= 1) || query.cursor.is_some(),
        "invalid-cursor: timeline pages require a cursor"
    );
    vault.read(Db::Archive, move |s| timeline_page(s, &query, i64::MAX))
}

pub(super) fn timeline_page(
    s: &Session<'_>,
    query: &HistoryQuery,
    upper_id: i64,
) -> Result<HistoryQueryResponse> {
    let limit = query.limit.unwrap_or(150).clamp(1, 1000) as usize;
    let capacity = limit as i64 + 1;
    let mut items = Vec::new();
    if let Some((time, id)) = cursor(query)?
        && time >= query.start_time_ms.unwrap_or(i64::MIN)
        && time <= query.end_time_ms.unwrap_or(i64::MAX)
    {
        let (sql, args) = timeline_ties_sql(query, capacity, upper_id, (time, id))?;
        items = s.query(&sql, args, history_entry_from_row)?;
    }
    if items.len() < capacity as usize {
        let (sql, args) = timeline_sql(query, capacity - items.len() as i64, upper_id)?;
        items.extend(s.query(&sql, args, history_entry_from_row)?);
    }
    let more = items.len() > limit;
    items.truncate(limit);
    let total =
        if query.include_total.unwrap_or(true) { Some(timeline_total(s, query)?) } else { None };
    let next_cursor =
        more.then(|| items.last().map(|v| format!("{}|{}", v.visit_time, v.id))).flatten();
    Ok(HistoryQueryResponse {
        total: total.unwrap_or(items.len()),
        total_exact: total.is_some(),
        page: 1,
        page_size: limit,
        page_count: total.map_or(1, |n| page_count(n, limit)),
        has_previous: query.cursor.is_some(),
        has_next: more,
        next_cursor,
        items,
        ..HistoryQueryResponse::default()
    })
}

/// A tuple inequality is only a residual filter in Turso 0.8.1. Split ties from
/// the remaining time range so both queries seek past the already-read prefix.
pub(super) fn timeline_sql(
    query: &HistoryQuery,
    limit: i64,
    upper: i64,
) -> Result<(String, Vec<Value>)> {
    let (mut start, mut end) =
        (query.start_time_ms.unwrap_or(i64::MIN), query.end_time_ms.unwrap_or(i64::MAX));
    let (mut lower, mut upper_op) = (">=", "<=");
    if let Some((time, _)) = cursor(query)? {
        if query.sort.as_deref() == Some("oldest") {
            if time >= start {
                start = time;
                lower = ">";
            }
        } else if time <= end {
            end = time;
            upper_op = "<";
        }
    }
    timeline_statement(
        query,
        limit,
        upper,
        &format!(
            "visits.visit_time_ms {lower} ?1 AND visits.visit_time_ms {upper_op} ?2 AND visits.id<=?6"
        ),
        start,
        end,
    )
}

fn timeline_ties_sql(
    query: &HistoryQuery,
    limit: i64,
    upper: i64,
    cursor: (i64, i64),
) -> Result<(String, Vec<Value>)> {
    let oldest = query.sort.as_deref() == Some("oldest");
    let (comparison, id, watermark) = if oldest {
        (">", cursor.1, upper)
    } else if cursor.1 <= upper {
        ("<", cursor.1, i64::MAX)
    } else {
        ("<=", upper, i64::MAX)
    };
    timeline_statement(
        query,
        limit,
        watermark,
        &format!(
            "visits.visit_time_ms=?1 AND visits.id {comparison} ?2 {}",
            if oldest && watermark != i64::MAX { "AND visits.id<=?6" } else { "" }
        ),
        cursor.0,
        id,
    )
}

fn cursor(query: &HistoryQuery) -> Result<Option<(i64, i64)>> {
    query
        .cursor
        .as_deref()
        .map(|raw| {
            parse_history_cursor(Some(raw))
                .and_then(HistoryCursor::chronological)
                .context("invalid-cursor: expected a timeline cursor")
        })
        .transpose()
}

fn timeline_statement(
    query: &HistoryQuery,
    limit: i64,
    upper: i64,
    bounds: &str,
    first: i64,
    second: i64,
) -> Result<(String, Vec<Value>)> {
    let order = if query.sort.as_deref() == Some("oldest") { "ASC" } else { "DESC" };
    let (index, profile) = if query.profile_id.is_some() {
        (
            "idx_visits_visible_profile_time_id",
            "AND visits.source_profile_id=(SELECT id FROM source_profiles WHERE profile_key=?3)",
        )
    } else {
        ("idx_visits_visible_time_id", "AND ?3 IS NULL")
    };
    let sql=format!("SELECT visits.id,source_profiles.profile_key,urls.url,urls.title,visits.visit_time_ms,visits.visit_duration_ms,visits.transition_type,visits.source_visit_id,visits.app_id
        FROM visits INDEXED BY {index}
        CROSS JOIN urls ON urls.id=visits.url_id
        CROSS JOIN source_profiles ON source_profiles.id=visits.source_profile_id
        WHERE visits.reverted_at IS NULL AND {bounds}
        {profile} AND (?4 IS NULL OR source_profiles.browser_kind=?4)
        AND (?5 IS NULL OR urls.url LIKE ?5)
        ORDER BY visits.visit_time_ms {order},visits.id {order} LIMIT ?7");
    let args = params![
        first,
        second,
        query.profile_id.as_deref(),
        query.browser_kind.as_deref(),
        query.domain.as_ref().filter(|d| !d.trim().is_empty()).map(|d| format!("%{d}%")),
        upper,
        limit
    ]
    .into_iter()
    .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((sql, args))
}

fn timeline_total(s: &Session<'_>, query: &HistoryQuery) -> Result<usize> {
    if query.profile_id.is_none()
        && query.browser_kind.is_none()
        && query.domain.is_none()
        && query.start_time_ms.is_none()
        && query.end_time_ms.is_none()
    {
        return Ok(s.query_one("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
            Ok(r.get::<i64>(0)?)
        })? as usize);
    }
    if query.domain.is_none() && query.start_time_ms.is_none() && query.end_time_ms.is_none() {
        return Ok(s.query_one("SELECT COALESCE(SUM(COALESCE((SELECT value FROM archive_totals WHERE name='sourceVisits:'||p.id),0)),0) FROM source_profiles p WHERE (?1 IS NULL OR p.profile_key=?1) AND (?2 IS NULL OR p.browser_kind=?2)",params![query.profile_id.as_deref(),query.browser_kind.as_deref()],|r|Ok(r.get::<i64>(0)?))? as usize);
    }
    let mut all = query.clone();
    all.cursor = None;
    let (sql, mut args) = timeline_sql(&all, 1, i64::MAX)?;
    let from = sql.find("FROM visits").unwrap();
    let end = sql.find("ORDER BY").unwrap();
    args.pop();
    Ok(
        s.query_one(
            &format!("SELECT COUNT(*) {}", &sql[from..end]),
            args,
            |r| Ok(r.get::<i64>(0)?),
        )? as usize,
    )
}

pub(super) fn history_entry_from_row(row: &vault_store::Row) -> vault_store::Result<HistoryEntry> {
    let url: String = row.get(2)?;
    let time: i64 = row.get(4)?;
    Ok(HistoryEntry {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        domain: url_domain(&url),
        url,
        title: row.get(3)?,
        visited_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(time)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        visit_time: time,
        duration_ms: row.get(5)?,
        transition: row.get(6)?,
        source_visit_id: row
            .get::<Option<String>>(7)?
            .and_then(|s| s.parse().ok())
            .unwrap_or_default(),
        app_id: row.get(8)?,
        favicon: None,
        enrichment_excerpt: None,
        visit_count: None,
    })
}
