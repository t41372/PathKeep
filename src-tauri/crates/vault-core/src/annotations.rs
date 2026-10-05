//! Responsibilities: preserve exact-URL notes, tags and their search documents.
//! Not responsible for: ranking, browser facts or opening database connections.
//! Failure modes: whitespace/limits drift, tags lose order or provenance, clears
//! remove unrelated data, search/outbox failure partially saves a user edit, cursors
//! omit tied rows, or literal percent/underscore text becomes a SQL wildcard.
//! Tests: annotations::tests and archive::search_documents::tests.

use crate::{
    Vault,
    models::{ReplaceTagsRequest, SetNotesRequest, UrlAnnotation},
    utils::now_rfc3339,
};
use anyhow::Result;
use std::collections::{BTreeSet, HashMap};
use vault_store::{Db, Priority, Session, params};

const MAX_NOTES_BYTES: usize = 16 * 1024;
const MAX_TAG_BYTES: usize = 64;
const MAX_TAGS_PER_URL: usize = 64;

/// A note belongs to the URL so it follows visits across browser profiles.
pub fn get_annotation(vault: &Vault, url: &str) -> Result<Option<UrlAnnotation>> {
    let url = url.to_owned();
    Ok(vault.store().read(Db::Archive, move |s| read_annotation(s, &url))?)
}

/// Saving a note also makes its searchable representation durable.
pub fn set_notes(vault: &Vault, request: SetNotesRequest) -> Result<UrlAnnotation> {
    anyhow::ensure!(!request.url.trim().is_empty(), "url is required");
    let notes = request.notes.trim().to_owned();
    anyhow::ensure!(
        notes.len() <= MAX_NOTES_BYTES,
        "notes body exceeds {MAX_NOTES_BYTES} bytes (received {} bytes)",
        notes.len()
    );
    Ok(vault.store().write(Db::Archive, Priority::Interactive, move |s| {
        if notes.is_empty() {
            s.execute("DELETE FROM url_annotations WHERE url=?1", [request.url.as_str()])?;
        } else {
            s.execute(
                "INSERT INTO url_annotations(url,notes,created_at,updated_at,source_profile,canonical_url)
                 VALUES(?1,?2,?3,?3,?4,?5) ON CONFLICT(url) DO UPDATE SET
                 notes=excluded.notes,updated_at=excluded.updated_at,
                 source_profile=COALESCE(excluded.source_profile,url_annotations.source_profile)",
                params![request.url.as_str(), notes, now_rfc3339(), request.source_profile, crate::pages::canonical_key(&request.url)],
            )?;
        }
        crate::archive::search_documents::annotations_changed(s, &request.url)?;
        annotation_or_empty(s, &request.url)
    })?)
}

/// Replacing tags preserves surviving tags' original order and provenance.
pub fn replace_tags(vault: &Vault, request: ReplaceTagsRequest) -> Result<UrlAnnotation> {
    anyhow::ensure!(!request.url.trim().is_empty(), "url is required");
    let normalized = normalize_tags(&request.tags)?;
    Ok(vault.store().write(Db::Archive, Priority::Interactive, move |s| {
        let kept: HashMap<String, (String, Option<String>)> = s
            .query(
                "SELECT tag,created_at,source_profile FROM url_tags WHERE url=?1",
                [request.url.as_str()],
                |r| {
                    Ok((
                        r.get::<String>(0)?.to_lowercase(),
                        (r.get::<String>(1)?, r.get::<Option<String>>(2)?),
                    ))
                },
            )?
            .into_iter()
            .collect();
        s.execute("DELETE FROM url_tags WHERE url=?1", [request.url.as_str()])?;
        let now = now_rfc3339();
        for tag in normalized {
            let (created, profile) = kept
                .get(&tag.to_lowercase())
                .cloned()
                .unwrap_or_else(|| (now.clone(), request.source_profile.clone()));
            s.execute(
                "INSERT INTO url_tags(url,tag,created_at,source_profile,canonical_url) VALUES(?1,?2,?3,?4,?5)",
                params![request.url.as_str(), tag, created, profile, crate::pages::canonical_key(&request.url)],
            )?;
        }
        crate::archive::search_documents::annotations_changed(s, &request.url)?;
        annotation_or_empty(s, &request.url)
    })?)
}

/// A page is never silently truncated: pass next_cursor to reach the remaining URLs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationPage {
    pub items: Vec<UrlAnnotation>,
    pub next_cursor: Option<String>,
}

pub fn list_annotations(
    vault: &Vault,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<AnnotationPage> {
    search_annotations(vault, "", limit, cursor)
}

/// Notes match by substring; the stable (updated_at, URL) cursor includes ties.
pub fn search_annotations(
    vault: &Vault,
    query: &str,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<AnnotationPage> {
    let cursor: Option<(String, String)> =
        cursor.map(|text| serde_json::from_str(&text)).transpose()?;
    let pattern = format!(
        "%{}%",
        query.trim().to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
    );
    let all = query.trim().is_empty();
    let size = limit.unwrap_or(if all { 500 } else { 200 });
    anyhow::ensure!(
        size > 0 && size < (i64::MAX as usize),
        "annotation page size must be positive and fit in an integer"
    );
    vault.read(Db::Archive, move |s| {
        let (at, url) = cursor.unwrap_or_default();
        let mut rows = s.query(
            "SELECT url,updated_at FROM (
            SELECT url,MAX(updated_at) AS updated_at FROM (
                SELECT url,updated_at FROM url_annotations WHERE ?1=1 OR LOWER(notes) LIKE ?2 ESCAPE '\\'
                UNION ALL SELECT url,created_at FROM url_tags WHERE ?1=1
            ) GROUP BY url
        ) WHERE ?3='' OR updated_at<?3 OR (updated_at=?3 AND url>?4)
        ORDER BY updated_at DESC,url LIMIT ?5",
            params![i64::from(all), pattern, at, url, (size + 1) as i64],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )?;
        let more = rows.len() > size;
        rows.truncate(size);
        let next_cursor = if more {
            rows.last().map(|(url, at)| serde_json::to_string(&(at, url))).transpose()?
        } else {
            None
        };
        let items = bundles(s, rows.into_iter().map(|(url, _)| url).collect())?;
        Ok(AnnotationPage { items, next_cursor })
    })
}

fn bundles(s: &Session<'_>, urls: Vec<String>) -> vault_store::Result<Vec<UrlAnnotation>> {
    urls.into_iter().map(|url| annotation_or_empty(s, &url)).collect()
}

fn annotation_or_empty(s: &Session<'_>, url: &str) -> vault_store::Result<UrlAnnotation> {
    Ok(read_annotation(s, url)?
        .unwrap_or_else(|| UrlAnnotation { url: url.into(), ..UrlAnnotation::default() }))
}

fn read_annotation(s: &Session<'_>, url: &str) -> vault_store::Result<Option<UrlAnnotation>> {
    let note = s
        .query(
            "SELECT notes,created_at,updated_at,source_profile FROM url_annotations WHERE url=?1",
            [url],
            |r| {
                Ok(UrlAnnotation {
                    url: url.into(),
                    notes: r.get(0)?,
                    created_at: r.get(1)?,
                    updated_at: r.get(2)?,
                    source_profile: r.get(3)?,
                    tags: Vec::new(),
                })
            },
        )?
        .pop();
    let tags =
        s.query("SELECT tag FROM url_tags WHERE url=?1 ORDER BY created_at,tag", [url], |r| {
            Ok(r.get::<String>(0)?)
        })?;
    if note.is_none() && tags.is_empty() {
        return Ok(None);
    }
    Ok(Some(UrlAnnotation {
        tags,
        ..note.unwrap_or_else(|| UrlAnnotation { url: url.into(), ..UrlAnnotation::default() })
    }))
}
fn normalize_tags(input: &[String]) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for raw in input {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > MAX_TAG_BYTES {
            anyhow::bail!(
                "tag `{trimmed}` exceeds {MAX_TAG_BYTES} bytes ({} bytes)",
                trimmed.len()
            );
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
        if out.len() > MAX_TAGS_PER_URL {
            anyhow::bail!("annotation exceeds {MAX_TAGS_PER_URL} tags");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod search_tests;
