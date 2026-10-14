//! Responsibilities: seek source URL tables by rowid and retain the original run filter.
//! Not responsible for: archive writes or persisting the cursor returned to consumers.
//! Failure modes: timestamps reorder unread rows, sync adds parents behind the scan,
//! resume scans every visit after zero, or per-page ORDER BY sorts the source table.
//! Tests: real-schema query plans and archive resume tests.
use crate::{
    HistoryBatchConsumer, ParsedUrl, SourceEvidenceChunk, StreamHistoryError, types::UrlScan,
};
use rusqlite::{Connection, Row, types::Value};
#[derive(Clone, Copy)]
pub(crate) enum Family {
    Chromium,
    Firefox,
    Safari,
}
impl Family {
    fn tables(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Chromium => ("urls", "visits", "url"),
            Self::Firefox => ("moz_places", "moz_historyvisits", "place_id"),
            Self::Safari => ("history_items", "history_visits", "history_item"),
        }
    }
    fn columns(self) -> &'static str {
        match self {
        Self::Chromium=>"urls.id,urls.url,urls.title,urls.visit_count,urls.typed_count,urls.last_visit_time,urls.hidden",
        Self::Firefox=>"moz_places.id,moz_places.url,moz_places.title,moz_places.visit_count,COALESCE(moz_places.hidden,0),COALESCE(moz_places.last_visit_date,0)",
        Self::Safari=>"history_items.id,history_items.url,
          (SELECT title FROM history_visits WHERE history_item=history_items.id AND title IS NOT NULL
           AND visit_time=(SELECT MAX(visit_time) FROM history_visits WHERE history_item=history_items.id AND title IS NOT NULL)
           ORDER BY id DESC LIMIT 1) AS title,
          (SELECT COUNT(*) FROM history_visits WHERE history_item=history_items.id) AS visit_count,
          (SELECT MAX(visit_time) FROM history_visits WHERE history_item=history_items.id) AS last_visit_time",
    }
    }
    fn time_filter(self) -> &'static str {
        match self {
            Self::Chromium => "urls.last_visit_time",
            Self::Firefox => "COALESCE(moz_places.last_visit_date,0)",
            Self::Safari => {
                "(SELECT MAX(visit_time) FROM history_visits WHERE history_item=history_items.id)"
            }
        }
    }
    fn time_value(self, time: i64) -> Value {
        match self {
            Self::Chromium => Value::Integer(time),
            Self::Firefox => Value::Integer(time.saturating_mul(1000)),
            Self::Safari => Value::Real(time.max(0) as f64 / 1000.0 - 978_307_200.0),
        }
    }
    fn entity(self) -> &'static str {
        match self {
            Self::Chromium => "chromium-url-row",
            Self::Firefox => "firefox-place-row",
            Self::Safari => "safari-history-item-row",
        }
    }
}

pub(crate) fn page_query(
    family: Family,
    scan: UrlScan,
    new_visits: Option<i64>,
    after: i64,
    size: usize,
) -> (String, Vec<Value>) {
    let (urls, visits, parent) = family.tables();
    let mut values = vec![Value::Integer(after), Value::Integer(scan.after_id)];
    let base = if scan.full {
        if matches!(family, Family::Safari) {
            format!("EXISTS(SELECT 1 FROM {visits} v WHERE v.{parent}={urls}.id)")
        } else {
            "1=1".into()
        }
    } else {
        values.extend([
            family.time_value(scan.time_floor),
            Value::Integer(scan.visit_floor),
            Value::Integer(scan.visit_ceiling),
        ]);
        format!(
            "({}>=?3 OR EXISTS(SELECT 1 FROM {visits} v WHERE v.{parent}={urls}.id AND v.id>?4 AND v.id<=?5))",
            family.time_filter()
        )
    };
    let mut filter = format!("({urls}.id>?2 AND {base})");
    if let Some(floor) = new_visits {
        values.push(Value::Integer(floor));
        filter = format!(
            "({filter} OR EXISTS(SELECT 1 FROM {visits} v WHERE v.{parent}={urls}.id AND v.id>?{}))",
            values.len()
        );
    }
    values.push(Value::Integer(size as i64));
    (
        format!(
            "SELECT {} FROM {urls} WHERE {urls}.id>?1 AND {filter} ORDER BY {urls}.id LIMIT ?{}",
            family.columns(),
            values.len()
        ),
        values,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn stream<C: HistoryBatchConsumer>(
    db: &Connection,
    family: Family,
    saved: Option<UrlScan>,
    time_floor: i64,
    visit_floor: i64,
    size: usize,
    consumer: &mut C,
    native: &mut Vec<crate::NativeEntity>,
    parse: fn(&Row<'_>) -> rusqlite::Result<ParsedUrl>,
) -> Result<(), StreamHistoryError<C::Error>> {
    let (_, visits, _) = family.tables();
    let ceiling = db
        .query_row(&format!("SELECT COALESCE(MAX(id),0) FROM {visits}"), [], |r| r.get::<_, i64>(0))
        .map_err(crate::ParseError::from)?;
    let scan = saved.unwrap_or(UrlScan {
        after_id: 0,
        time_floor,
        visit_floor,
        visit_ceiling: ceiling,
        full: time_floor == 0 && visit_floor == 0,
    });
    consumer.begin_urls(scan).map_err(StreamHistoryError::Consumer)?;
    // Only visits added since this URL phase started can introduce parents behind
    // its position. Previously staged visits already have a committed or upcoming URL.
    let new_floor = visit_floor.max(scan.visit_ceiling);
    let new_visits = (ceiling > new_floor).then_some(new_floor);
    let mut after = if new_visits.is_some() { 0 } else { scan.after_id };
    loop {
        let (sql, values) = page_query(family, scan, new_visits, after, size.max(1));
        let mut statement = db.prepare(&sql).map_err(crate::ParseError::from)?;
        let columns = statement.column_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut rows =
            statement.query(rusqlite::params_from_iter(values)).map_err(crate::ParseError::from)?;
        let mut batch = Vec::new();
        let mut evidence = SourceEvidenceChunk::default();
        while let Some(row) = rows.next().map_err(crate::ParseError::from)? {
            let url = parse(row).map_err(crate::ParseError::from)?;
            after = url.source_url_id;
            batch.push(url);
            evidence.native_entities.push(
                crate::observation::capture_native_row(row, &columns, family.entity(), "id", None)
                    .map_err(crate::ParseError::from)?,
            );
        }
        if batch.is_empty() {
            break;
        }
        consumer.urls(batch).map_err(StreamHistoryError::Consumer)?;
        if consumer.retain_source_evidence_in_report() {
            native.extend(evidence.native_entities);
        } else {
            consumer.source_evidence(evidence).map_err(StreamHistoryError::Consumer)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
