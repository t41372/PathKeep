//! Responsibilities: persist and query the canonical key shared by URL variants.
//! Not responsible for: browser observations, fetching, stars or ranking.
//! Failure modes: writers omit normalization; a variant lookup scans by path prefix.
//! Tests: pages::tests against production SQL on Turso.
use vault_store::{Result, Session};
pub(crate) const PAGE_IDS_SQL: &str = "SELECT id FROM pages WHERE canonical_url=?1 ORDER BY id";
pub(crate) const URL_IDS_SQL: &str =
    "SELECT u.id FROM pages p JOIN urls u ON u.page_id=p.id WHERE p.canonical_url=?1";
pub(crate) fn canonical_key(url: &str) -> String {
    crate::visit_taxonomy::normalize_visit_url(url)
        .map(|value| value.canonical_url)
        .unwrap_or_else(|| url.trim().to_owned())
}
pub(crate) fn ids_for_key(s: &Session<'_>, key: &str) -> Result<Vec<i64>> {
    s.query(PAGE_IDS_SQL, [key], |r| Ok(r.get(0)?))
}
#[cfg(test)]
mod tests;
