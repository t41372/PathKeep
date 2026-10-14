//! Responsibilities: encode typed keyset cursors and expose the extra-row continuation.
//! Not responsible for: query order, database access or selecting page sizes.
//! Failure modes: tied sort keys skip records, malformed cursors become first pages,
//! or the lookahead record is returned twice. Tests: intelligence_paging.
use crate::models::IntelligencePage;
use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
pub(super) fn cursor<T: DeserializeOwned>(value: Option<&str>) -> Result<Option<T>> {
    value.map(serde_json::from_str).transpose().map_err(Into::into)
}
pub(super) fn finish<T, C: Serialize>(
    mut rows: Vec<(T, C)>,
    limit: usize,
) -> Result<IntelligencePage<T>> {
    let next_cursor = if rows.len() > limit {
        rows.pop();
        Some(serde_json::to_string(&rows.last().unwrap().1)?)
    } else {
        None
    };
    Ok(IntelligencePage { items: rows.into_iter().map(|(item, _)| item).collect(), next_cursor })
}
