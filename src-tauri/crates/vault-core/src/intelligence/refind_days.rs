//! Responsibilities: keep, for each profile and local day, the refind pages opened on it
//! (`refind_day_pages_rows`), and answer which were opened in a range and how many.
//! Not responsible for: deciding which pages are refind pages (`intelligence_structural_aggregates`)
//! or the order the day page lists them in (`intelligence_refind`).
//! Failure modes: a page opened on several days of a range counts once per day; the same page in
//! two profiles counts twice with no profile chosen; an append drops a day's earlier pages; a
//! range read walks one row per page and day (a year was 3.3 s at 14.4M visits).
//!
//! One row per profile and day holds the day's pages as a blob of sorted 64-bit keys, a hash of
//! the canonical URL (`page_key`, also on the page's `refind_pages_rows` row). A range reads one
//! row per day and profile and unions the keys. Two different pages share a key with a chance
//! of about 3M² / 2⁶⁵ ≈ 2×10⁻⁷ for 3 million refind pages.

use crate::models::DateRange;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use vault_store::{Session, params_owned as params};

/// The key a refind page is filed under: the first 8 bytes of the SHA-256 of its canonical URL.
pub(super) fn page_key(canonical_url: &str) -> i64 {
    let digest = Sha256::digest(canonical_url.as_bytes());
    i64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
}

fn encode(keys: &[i64]) -> Vec<u8> {
    keys.iter().flat_map(|key| key.to_le_bytes()).collect()
}

fn decode(blob: &[u8]) -> impl Iterator<Item = i64> + '_ {
    blob.chunks_exact(8).map(|bytes| i64::from_le_bytes(bytes.try_into().expect("8 bytes")))
}

/// Adds pages to their days for one profile's generation, inside the caller's transaction, and
/// returns how many day rows it created. A day already stored keeps its pages.
pub(super) fn add(
    s: &Session<'_>,
    generation: i64,
    profile_id: &str,
    days: BTreeMap<String, Vec<i64>>,
) -> Result<usize> {
    let mut created = 0;
    for (day, mut keys) in days {
        let stored = s.query(
            "SELECT page_keys FROM refind_day_pages_rows
             WHERE generation=?1 AND profile_id=?2 AND date_key=?3",
            params![generation, profile_id, day.as_str()],
            |r| Ok(r.get::<Vec<u8>>(0)?),
        )?;
        match stored.first() {
            Some(blob) => keys.extend(decode(blob)),
            None => created += 1,
        }
        keys.sort_unstable();
        keys.dedup();
        s.execute(
            "INSERT OR REPLACE INTO refind_day_pages_rows(generation,profile_id,date_key,page_keys)
             VALUES(?1,?2,?3,?4)",
            params![generation, profile_id, day, encode(&keys)],
        )?;
    }
    Ok(created)
}

/// The published generation of each profile in scope: every profile, or the one asked for.
fn generations(s: &Session<'_>, profile_id: Option<&str>) -> Result<Vec<(String, i64)>> {
    Ok(s.query(
        "SELECT profile_id, aggregates FROM intelligence_generations
         WHERE ?1 IS NULL OR profile_id=?1",
        params![profile_id],
        |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
    )?)
}

/// For each day of `range` in each profile in scope, the keys of the refind pages opened on it.
fn for_each_day(
    s: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
    mut visit: impl FnMut(&str, &[u8]),
) -> Result<()> {
    for (profile, generation) in generations(s, profile_id)? {
        let blobs = s.query(
            "SELECT page_keys FROM refind_day_pages_rows
             WHERE generation=?1 AND profile_id=?2 AND date_key>=?3 AND date_key<=?4",
            params![generation, profile.as_str(), range.start.as_str(), range.end.as_str()],
            |r| Ok(r.get::<Vec<u8>>(0)?),
        )?;
        for blob in blobs {
            visit(&profile, &blob);
        }
    }
    Ok(())
}

/// How many refind pages were opened on a day of `range`: each page once, across profiles when
/// none is chosen.
pub(super) fn count(s: &Session<'_>, range: &DateRange, profile_id: Option<&str>) -> Result<i64> {
    let mut pages = HashSet::new();
    for_each_day(s, range, profile_id, |_, blob| pages.extend(decode(blob)))?;
    Ok(pages.len() as i64)
}

/// The refind pages opened on a day of `range`, as `(profile, key)`: one per page and profile.
pub(super) fn opened(
    s: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<Vec<(String, i64)>> {
    let mut pages = HashSet::new();
    for_each_day(s, range, profile_id, |profile, blob| {
        pages.extend(decode(blob).map(|key| (profile.to_owned(), key)));
    })?;
    let mut pages = pages.into_iter().collect::<Vec<_>>();
    pages.sort_unstable();
    Ok(pages)
}

/// The days each page of `pages` adds, as day → keys, for `add`.
pub(super) fn by_day<'a>(
    pages: impl IntoIterator<Item = &'a super::RefindPageRecord>,
) -> BTreeMap<String, Vec<i64>> {
    let mut days = BTreeMap::<String, Vec<i64>>::new();
    for page in pages {
        let key = page_key(&page.canonical_url);
        for day in &page.days {
            days.entry(day.clone()).or_default().push(key);
        }
    }
    days
}
