//! Per-URL annotations: notes and tags written from the Browse detail panel.
//!
//! Notes and tags travel with the URL, not with the visit, so a note written
//! on one visit is visible the next time the user opens the same page in
//! Explorer. Storage lives in two tables (url_annotations + url_tags) created
//! by migration 011 — the split keeps tag lookup cheap (`WHERE tag = ?`) and
//! lets the notes column compress well even when many URLs have empty notes.
//!
//! ## Responsibilities
//! - Get / set the per-URL annotation bundle (`UrlAnnotation`).
//! - Replace, add, and remove individual tags.
//! - List and search annotations for the future export / FTS surfaces.
//!
//! ## Not responsible for
//! - Visit-level metadata (transition kind, durations) — stays in
//!   `archive::history`.
//! - Tag normalization beyond trim + de-dupe — UI owns the affordances.
//! - Cross-URL search ranking — search returns LIKE matches now; a real
//!   index lives in a future migration.
//!
//! ## Performance notes
//! - All operations are single-statement; no full-table scans except the
//!   list/search helpers, which are explicit reads.
//! - Tag list lookup is indexed by `(tag, url)`; notes row lookup hits the
//!   primary key.

use crate::{
    archive::open_archive_connection,
    config::ProjectPaths,
    models::{AppConfig, ReplaceTagsRequest, SetNotesRequest, UrlAnnotation},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

/// Maximum notes body the backend accepts in a single write. Keeps a single
/// runaway paste from bloating the row past the SQLite payload limits.
const MAX_NOTES_BYTES: usize = 16 * 1024;

/// Maximum tag length in bytes after trim. Empty tags are filtered out.
const MAX_TAG_BYTES: usize = 64;

/// Maximum tags per URL — the design uses paper chips and the row truncates
/// past about a dozen; the limit prevents abuse.
const MAX_TAGS_PER_URL: usize = 64;

/// Reads the annotation bundle for a URL. Returns `None` when there is no
/// notes row and no tags — callers should treat that as "no annotation".
pub fn get_annotation(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    url: &str,
) -> Result<Option<UrlAnnotation>> {
    let connection = open_archive_connection(paths, config, key)?;
    read_annotation(&connection, url)
}

/// Sets or clears the notes body for a URL. An empty body (after trim)
/// removes the notes row when there are no tags left to keep the URL alive.
pub fn set_notes(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: SetNotesRequest,
) -> Result<UrlAnnotation> {
    if request.url.trim().is_empty() {
        anyhow::bail!("url is required");
    }
    let trimmed = request.notes.trim();
    if trimmed.len() > MAX_NOTES_BYTES {
        anyhow::bail!(
            "notes body exceeds {MAX_NOTES_BYTES} bytes (received {} bytes)",
            trimmed.len()
        );
    }
    let connection = open_archive_connection(paths, config, key)?;
    let now = now_rfc3339();
    if trimmed.is_empty() {
        connection
            .execute("DELETE FROM url_annotations WHERE url = ?1", params![request.url])
            .context("clearing url_annotations row")?;
    } else {
        connection
            .execute(
                r#"INSERT INTO url_annotations(url, notes, created_at, updated_at, source_profile)
                   VALUES(?1, ?2, ?3, ?3, ?4)
                   ON CONFLICT(url) DO UPDATE SET
                     notes = excluded.notes,
                     updated_at = excluded.updated_at,
                     source_profile = COALESCE(excluded.source_profile, url_annotations.source_profile)"#,
                params![
                    request.url,
                    trimmed,
                    now,
                    request.source_profile,
                ],
            )
            .context("writing url_annotations row")?;
    }
    Ok(read_annotation(&connection, &request.url)?
        .unwrap_or_else(|| UrlAnnotation { url: request.url.clone(), ..UrlAnnotation::default() }))
}

/// Replaces the full tag set for a URL. Tags are trimmed and de-duplicated
/// (case-insensitive) before persistence; passing an empty list removes
/// every tag for the URL.
pub fn replace_tags(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ReplaceTagsRequest,
) -> Result<UrlAnnotation> {
    if request.url.trim().is_empty() {
        anyhow::bail!("url is required");
    }
    let normalized = normalize_tags(&request.tags)?;
    let mut connection = open_archive_connection(paths, config, key)?;
    let now = now_rfc3339();
    // Replace must be atomic: a delete-then-insert sequence that errors
    // half-way through (constraint violation, disk-full, etc.) would
    // silently drop the user's tag list. Wrap the whole replacement in a
    // transaction so either the new tag list lands or nothing changes.
    {
        let tx = connection.transaction().context("opening replace_tags transaction")?;
        tx.execute("DELETE FROM url_tags WHERE url = ?1", params![request.url])
            .context("clearing url_tags for replacement")?;
        if !normalized.is_empty() {
            let mut statement = tx.prepare(
                r#"INSERT INTO url_tags(url, tag, created_at, source_profile)
                   VALUES(?1, ?2, ?3, ?4)"#,
            )?;
            for tag in &normalized {
                statement
                    .execute(params![request.url, tag, now, request.source_profile])
                    .with_context(|| format!("inserting url_tags row for `{tag}`"))?;
            }
        }
        tx.commit().context("committing replace_tags transaction")?;
    }
    Ok(read_annotation(&connection, &request.url)?
        .unwrap_or_else(|| UrlAnnotation { url: request.url.clone(), ..UrlAnnotation::default() }))
}

/// Lists every URL that has at least one annotation (note or tag), sorted by
/// most-recent notes-update first. Returns full bundles, including tags.
pub fn list_annotations(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<UrlAnnotation>> {
    let connection = open_archive_connection(paths, config, key)?;
    let cap = limit.unwrap_or(500).min(5_000);
    let urls = collect_annotated_urls(&connection, cap)?;
    let mut out = Vec::with_capacity(urls.len());
    for url in &urls {
        if let Some(annotation) = read_annotation(&connection, url)? {
            out.push(annotation);
        }
    }
    Ok(out)
}

/// Returns annotations whose notes body contains the query substring
/// (case-insensitive). Cheap LIKE-based search; a proper FTS index can land
/// later without changing the call signature.
pub fn search_annotations(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<UrlAnnotation>> {
    let connection = open_archive_connection(paths, config, key)?;
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return list_annotations(paths, config, key, limit);
    }
    let cap = limit.unwrap_or(200).min(5_000);
    let pattern = format!("%{}%", trimmed.to_lowercase());
    let mut statement = connection.prepare(
        r#"SELECT url FROM url_annotations
           WHERE LOWER(notes) LIKE ?1
           ORDER BY updated_at DESC LIMIT ?2"#,
    )?;
    let urls: Vec<String> = statement
        .query_map(params![pattern, cap as i64], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("decoding search_annotations URL rows")?;
    let mut out = Vec::with_capacity(urls.len());
    for url in &urls {
        if let Some(annotation) = read_annotation(&connection, url)? {
            out.push(annotation);
        }
    }
    Ok(out)
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

fn collect_annotated_urls(connection: &Connection, cap: usize) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        r#"
        SELECT url FROM (
          SELECT url, updated_at FROM url_annotations
          UNION
          SELECT url, MAX(created_at) FROM url_tags GROUP BY url
        )
        GROUP BY url
        ORDER BY MAX(updated_at) DESC
        LIMIT ?1
        "#,
    )?;
    let rows: Vec<String> = statement
        .query_map(params![cap as i64], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("decoding annotated-URL rows")?;
    Ok(rows)
}

fn read_annotation(connection: &Connection, url: &str) -> Result<Option<UrlAnnotation>> {
    let row: Option<(String, String, String, Option<String>)> = connection
        .query_row(
            r#"SELECT notes, created_at, updated_at, source_profile
               FROM url_annotations WHERE url = ?1"#,
            params![url],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    let mut tag_statement = connection
        .prepare("SELECT tag FROM url_tags WHERE url = ?1 ORDER BY created_at ASC, tag ASC")?;
    let tags: Vec<String> = tag_statement
        .query_map(params![url], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("decoding url_tags rows")?;
    match (row, tags.is_empty()) {
        (Some((notes, created_at, updated_at, source_profile)), _) => Ok(Some(UrlAnnotation {
            url: url.to_string(),
            notes,
            tags,
            created_at,
            updated_at,
            source_profile,
        })),
        (None, false) => {
            Ok(Some(UrlAnnotation { url: url.to_string(), tags, ..UrlAnnotation::default() }))
        }
        (None, true) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{ProjectPaths, project_paths_with_root},
        models::{AppConfig, ArchiveMode},
    };
    use std::{
        fs,
        sync::atomic::{AtomicU32, Ordering},
    };

    static TEST_PATH_SEQ: AtomicU32 = AtomicU32::new(0);

    fn make_paths(label: &str) -> ProjectPaths {
        let seq = TEST_PATH_SEQ.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("pk-annot-{label}-{}-{seq}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        project_paths_with_root(&root)
    }

    fn ensure_schema(paths: &ProjectPaths, config: &AppConfig) {
        // open_archive_connection runs the migration pipeline on first open,
        // so simply opening + dropping the connection bootstraps the schema.
        let _ = open_archive_connection(paths, config, None).expect("schema bootstrap");
    }

    fn plaintext_config() -> AppConfig {
        AppConfig { archive_mode: ArchiveMode::Plaintext, ..AppConfig::default() }
    }

    #[test]
    fn returns_none_when_no_annotation_exists() {
        let paths = make_paths("none");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let result = get_annotation(&paths, &config, None, "https://example.com/").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn writes_and_reads_notes_round_trip() {
        let paths = make_paths("notes");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let bundle = set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: "https://example.com/x".into(),
                notes: "  hello world  ".into(),
                source_profile: Some("chrome:Default".into()),
            },
        )
        .unwrap();
        assert_eq!(bundle.notes, "hello world");
        assert_eq!(bundle.source_profile.as_deref(), Some("chrome:Default"));
        let reread = get_annotation(&paths, &config, None, "https://example.com/x")
            .unwrap()
            .expect("annotation should exist");
        assert_eq!(reread.notes, "hello world");
        assert!(!reread.created_at.is_empty());
    }

    #[test]
    fn empty_notes_clears_row_when_no_tags_remain() {
        let paths = make_paths("clear");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let url = "https://example.com/clear";
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest { url: url.into(), notes: "stay".into(), source_profile: None },
        )
        .unwrap();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest { url: url.into(), notes: "".into(), source_profile: None },
        )
        .unwrap();
        let after = get_annotation(&paths, &config, None, url).unwrap();
        assert!(after.is_none());
    }

    #[test]
    fn replace_tags_dedupes_and_trims() {
        let paths = make_paths("tags");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let url = "https://example.com/tagged";
        let bundle = replace_tags(
            &paths,
            &config,
            None,
            ReplaceTagsRequest {
                url: url.into(),
                tags: vec!["rust".into(), "  Rust ".into(), "".into(), "sqlite".into()],
                source_profile: None,
            },
        )
        .unwrap();
        assert_eq!(bundle.tags, vec!["rust".to_string(), "sqlite".to_string()]);
    }

    #[test]
    fn replace_tags_with_empty_list_removes_tags_but_keeps_notes() {
        let paths = make_paths("keep-notes");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let url = "https://example.com/mix";
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest { url: url.into(), notes: "keep me".into(), source_profile: None },
        )
        .unwrap();
        replace_tags(
            &paths,
            &config,
            None,
            ReplaceTagsRequest { url: url.into(), tags: vec!["one".into()], source_profile: None },
        )
        .unwrap();
        let after_set = get_annotation(&paths, &config, None, url).unwrap().unwrap();
        assert_eq!(after_set.tags, vec!["one"]);
        assert_eq!(after_set.notes, "keep me");
        replace_tags(
            &paths,
            &config,
            None,
            ReplaceTagsRequest { url: url.into(), tags: vec![], source_profile: None },
        )
        .unwrap();
        let after_clear = get_annotation(&paths, &config, None, url).unwrap().unwrap();
        assert!(after_clear.tags.is_empty());
        assert_eq!(after_clear.notes, "keep me");
    }

    #[test]
    fn search_finds_notes_substring_case_insensitive() {
        let paths = make_paths("search");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: "https://example.com/a".into(),
                notes: "Reading about Rust async runtimes".into(),
                source_profile: None,
            },
        )
        .unwrap();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: "https://example.com/b".into(),
                notes: "SQLite docs".into(),
                source_profile: None,
            },
        )
        .unwrap();
        let results = search_annotations(&paths, &config, None, "RUST", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com/a");
    }

    #[test]
    fn notes_exceeding_byte_limit_is_rejected() {
        let paths = make_paths("size");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let big = "x".repeat(MAX_NOTES_BYTES + 1);
        let err = set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: "https://example.com/big".into(),
                notes: big,
                source_profile: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("exceeds"));
    }

    #[test]
    fn tag_exceeding_length_is_rejected() {
        let paths = make_paths("longtag");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        let big_tag = "t".repeat(MAX_TAG_BYTES + 1);
        let err = replace_tags(
            &paths,
            &config,
            None,
            ReplaceTagsRequest {
                url: "https://example.com/longtag".into(),
                tags: vec![big_tag],
                source_profile: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("exceeds"));
    }

    #[test]
    fn list_returns_most_recent_first() {
        let paths = make_paths("list-order");
        let config = plaintext_config();
        ensure_schema(&paths, &config);
        for (url, note) in [
            ("https://example.com/1", "first"),
            ("https://example.com/2", "second"),
            ("https://example.com/3", "third"),
        ] {
            set_notes(
                &paths,
                &config,
                None,
                SetNotesRequest { url: url.into(), notes: note.into(), source_profile: None },
            )
            .unwrap();
        }
        let listed = list_annotations(&paths, &config, None, None).unwrap();
        assert_eq!(listed.len(), 3);
        let first_url = &listed[0].url;
        let last_url = &listed[2].url;
        assert!(first_url != last_url);
    }
}
