//! Responsibilities: preserve profile/page identity and canonical source facts.
//! Not responsible for: parsing, batch scheduling, evidence, run lifecycle, or deciding which
//! archived visit a captured row is (`identity`).
//! Failure modes: duplicate fingerprints, older metadata replacing newer observations, a browser
//! row id joining a new URL or download to an old one, unpublished page anchors, ignored visits
//! widening bounds, lost nullable fields or duplicate favicon bytes, a value the browser wrote
//! later never reaching a saved visit. Tests: dedup_scenarios* in both key modes.

use crate::archive::favicon_metadata::favicon_url_metadata;
use crate::archive::visit_event_fingerprint;
use crate::utils::{now_rfc3339, sha256_hex, unix_micros_to_chrome_time};
use crate::visit_taxonomy::registrable_domain_for_url;
use browser_history_parser::{
    ParsedDownload, ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit,
};
use std::collections::HashMap;
use vault_store::{Result, Session, params};

/// Buffers visit-time bounds per canonical URL so one post-pass can update URL ranges.
#[derive(Debug)]
pub(super) struct UrlVisitBounds {
    pub(super) first_visit_ms: i64,
    pub(super) first_visit_iso: String,
    pub(super) last_visit_ms: i64,
    pub(super) last_visit_iso: String,
}

/// Upserts the stable source-profile identity row and returns its canonical id.
pub(crate) fn upsert_source_profile(
    archive: &Session<'_>,
    profile: &crate::models::BrowserProfile,
) -> Result<i64> {
    let enabled = archive
        .query(
            "SELECT enabled FROM source_profiles WHERE profile_key=?1",
            [profile.profile_id.clone()],
            |r| Ok(r.get::<i64>(0)?),
        )?
        .pop();
    if enabled != Some(1) {
        crate::archive::totals::add(archive, "sourceProfiles", 1)?;
    }
    let browser_kind =
        profile.profile_id.split(':').next().unwrap_or(&profile.browser_family).to_string();
    let browser_product = if profile.browser_name.trim().is_empty() {
        browser_kind.clone()
    } else {
        profile.browser_name.clone()
    };
    archive.query_one(
        "INSERT INTO source_profiles (
           browser_kind,
           browser_family,
           browser_product,
           browser_version,
           profile_name,
           profile_path,
           discovered_at,
           enabled,
           profile_key,
           user_name,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?10)
         ON CONFLICT(profile_key) DO UPDATE SET
           browser_kind = excluded.browser_kind,
           browser_family = excluded.browser_family,
           browser_product = excluded.browser_product,
           browser_version = excluded.browser_version,
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           user_name = excluded.user_name,
           updated_at = excluded.updated_at,
           enabled = 1
         RETURNING id",
        params![
            browser_kind,
            profile.browser_family.clone(),
            browser_product,
            profile.browser_version.clone(),
            profile.profile_name.clone(),
            profile.profile_path.clone(),
            now_rfc3339(),
            profile.profile_id.clone(),
            profile.user_name.clone(),
            now_rfc3339(),
        ],
        |row| Ok(row.get(0)?),
    )
}

/// The page of `url`, added when no profile has seen it yet. Looked up first: most URLs of a
/// large import or backup already have their page, and an upsert would rewrite its row to read
/// the id.
fn page_for(archive: &Session<'_>, url: &str) -> Result<i64> {
    if let Some(page) =
        archive.query("SELECT id FROM pages WHERE url=?1", [url], |r| Ok(r.get::<i64>(0)?))?.pop()
    {
        return Ok(page);
    }
    Ok(archive
        .insert(
            "INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?3)",
            params![url, registrable_domain_for_url(url), crate::pages::canonical_key(url)],
        )?
        .expect("a plain INSERT adds its row"))
}

/// A profile's stored observation of one URL: what an upsert compares against.
struct StoredUrl {
    id: i64,
    page: i64,
    last_visit_ms: Option<i64>,
    visit_count: i64,
    typed_count: i64,
}

/// What `upsert_url` did to a profile's URL row.
pub(super) struct UpsertedUrl {
    pub(super) id: i64,
    pub(super) page: i64,
    /// The page the row had before; `None` for a new row.
    pub(super) previous_page: Option<i64>,
    /// Whether the row is new or took this observation's URL and title, the things a page's
    /// search document shows. Raised counts alone do not change it.
    pub(super) rewritten: bool,
}

/// Upserts one canonical URL row and returns its stable archive id, its page and the page it
/// had before (`None` for a new row). A profile's URL is its address: the browser's row id names
/// another address once the browser numbers its rows again, and version 1 joined by that id, so
/// one address can have more than one row; the first is used. A newer observation replaces the
/// row's title and last visit; an older one only raises its counts, and writes nothing when they
/// are not higher. Plain statements rather than `INSERT … ON CONFLICT … RETURNING`, which Turso
/// answers through a temporary file per row (`Session::insert`).
pub(super) fn upsert_url(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url: &ParsedUrl,
    payload_hash: &str,
) -> Result<UpsertedUrl> {
    let stored = archive
        .query(
            "SELECT id,page_id,last_visit_ms,visit_count,typed_count FROM urls INDEXED BY idx_urls_url
             WHERE url=?2 AND source_profile_id=?1 ORDER BY id LIMIT 1",
            params![source_profile_id, url.url.clone()],
            |r| {
                Ok(StoredUrl {
                    id: r.get(0)?,
                    page: r.get(1)?,
                    last_visit_ms: r.get(2)?,
                    visit_count: r.get(3)?,
                    typed_count: r.get(4)?,
                })
            },
        )?
        .pop();
    let Some(stored) = stored else {
        let page = page_for(archive, &url.url)?;
        let id = archive
            .insert(
                "INSERT INTO urls (url, title, visit_count, typed_count, first_visit_ms,
                   first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id,
                   created_by_run_id, source_url_id, hidden, payload_hash, recorded_at, page_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    url.url.clone(),
                    url.title.clone(),
                    url.visit_count,
                    url.typed_count,
                    url.last_visit_ms,
                    url.last_visit_iso.clone(),
                    source_profile_id,
                    run_id,
                    url.source_url_id,
                    url.hidden as i64,
                    payload_hash,
                    now_rfc3339(),
                    page,
                ],
            )?
            .expect("a plain INSERT adds its row");
        return Ok(UpsertedUrl { id, page, previous_page: None, rewritten: true });
    };
    if stored.last_visit_ms.is_some_and(|last| url.last_visit_ms > last) {
        archive.execute(
            "UPDATE urls SET title=?2, visit_count=MAX(visit_count,?3),
               typed_count=MAX(typed_count,?4), hidden=?5, payload_hash=?6, recorded_at=?7,
               last_visit_ms=?8, last_visit_iso=?9, source_url_id=?10
             WHERE id=?1",
            params![
                stored.id,
                url.title.clone(),
                url.visit_count,
                url.typed_count,
                url.hidden as i64,
                payload_hash,
                now_rfc3339(),
                url.last_visit_ms,
                url.last_visit_iso.clone(),
                url.source_url_id,
            ],
        )?;
        return Ok(UpsertedUrl {
            id: stored.id,
            page: stored.page,
            previous_page: Some(stored.page),
            rewritten: true,
        });
    }
    // An older observation keeps the current URL and page.
    if url.visit_count > stored.visit_count || url.typed_count > stored.typed_count {
        archive.execute(
            "UPDATE urls SET visit_count=MAX(visit_count,?2), typed_count=MAX(typed_count,?3)
             WHERE id=?1",
            params![stored.id, url.visit_count, url.typed_count],
        )?;
    }
    Ok(UpsertedUrl {
        id: stored.id,
        page: stored.page,
        previous_page: Some(stored.page),
        rewritten: false,
    })
}

/// A new visit that another source's copy already stands in for: it is written hidden behind
/// that copy (`archive::same_visit`).
pub(super) type HiddenBehind = Option<i64>;

/// Inserts one canonical visit row; its id, or `None` when a row without a browser time
/// (Takeout's) was already there. A row with a browser time is written once `identity` found no
/// archived visit it is.
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_visit(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
    hidden: HiddenBehind,
    parent: Option<i64>,
) -> Result<Option<i64>> {
    let fingerprint = visit_event_fingerprint(
        "chromium-history",
        &visit.url,
        unix_micros_to_chrome_time(visit.visit_time_ms.saturating_mul(1_000)),
        visit.title.as_deref(),
        visit.transition,
        visit.app_id.as_deref(),
    );
    write_visit(
        archive,
        run_id,
        source_profile_id,
        url_id,
        visit,
        payload_hash,
        &fingerprint,
        None,
        hidden,
        parent,
    )
}

/// Import origins preserve their existing fingerprint domain and batch identity.
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_import_visit(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
    origin: &super::consumer::ImportOrigin,
    hidden: HiddenBehind,
    parent: Option<i64>,
) -> Result<Option<i64>> {
    let fingerprint = visit_event_fingerprint(
        &origin.source_kind,
        &visit.url,
        visit.visit_time_ms,
        visit.title.as_deref(),
        visit.transition,
        visit.app_id.as_deref(),
    );
    write_visit(
        archive,
        run_id,
        source_profile_id,
        url_id,
        visit,
        payload_hash,
        &fingerprint,
        Some(origin.batch_id),
        hidden,
        parent,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_visit(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
    fingerprint: &str,
    batch_id: Option<i64>,
    hidden: HiddenBehind,
    parent: Option<i64>,
) -> Result<Option<i64>> {
    let hidden_at = hidden.map(|_| now_rfc3339());
    // A row with a browser time is identified by it (`identity`). Its fingerprint, built from the
    // millisecond, would make two visits less than a millisecond apart one, so it has none.
    let fingerprint = visit.source_visit_time.is_none().then(|| fingerprint.to_owned());
    archive.insert(
        "INSERT OR IGNORE INTO visits (
               url_id,
               source_visit_id,
               visit_time_ms,
               visit_time_iso,
               transition_type,
               visit_duration_ms,
               source_profile_id,
               created_by_run_id,
               from_visit,
               is_known_to_sync,
               visited_link_id,
               external_referrer_url,
               app_id,
               event_fingerprint,
               payload_hash,
               recorded_at,
               import_batch_id,
               same_as,
               reverted_at,
               reverted_by_run_id,
               source_visit_time,
               parent_visit_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19, CASE WHEN ?18 IS NULL THEN NULL ELSE ?8 END, ?20, ?21)",
        params![
            url_id,
            visit.source_visit_id.to_string(),
            visit.visit_time_ms,
            visit.visit_time_iso.clone(),
            visit.transition,
            visit.visit_duration_ms,
            source_profile_id,
            run_id,
            visit.from_visit,
            visit.is_known_to_sync as i64,
            visit.visited_link_id,
            visit.external_referrer_url.clone(),
            visit.app_id.clone(),
            fingerprint,
            payload_hash,
            now_rfc3339(),
            batch_id,
            hidden,
            hidden_at,
            visit.source_visit_time.clone(),
            parent,
        ],
    )
}

/// What a later read of a visit changes. Chrome writes a visit's duration when its tab closes,
/// sometimes days later; Sync fills in the visit that opened it; a row is marked known to Sync
/// once it is uploaded. The time and address never change. The duration only grows and the Sync
/// mark only turns on, so a read never takes them back: a replayed checkpoint is an older read,
/// and a visit the browser has since dropped could not be read again. The opener's row id, the transition
/// and the referrer stay as first read: Insights classified and grouped the visit by them, and
/// after a renumbering the opener's row id is in numbering the archived visits do not share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refresh {
    /// A visit an earlier capture saved: this read is the newest, and wins.
    Latest,
    /// Another row, in this capture, of a visit this capture wrote (a redirect's hops share a
    /// time): the visit lasted as long as its longest row, the first row's opener stays, and a
    /// parent either row names is kept.
    SameCapture,
}

/// Brings an archived visit up to this read of it. Writes nothing when the row's payload is the
/// one already saved, and never clears a parent: a later read that cannot name the opener (its
/// row expired from the browser) does not make the visit unopened. `stamp` gives a version 1
/// visit its browser time.
pub(super) fn refresh_visit(
    archive: &Session<'_>,
    archived: &super::identity::Archived,
    visit: &ParsedVisit,
    payload_hash: &str,
    parent: Option<i64>,
    refresh: Refresh,
    stamp: bool,
) -> Result<bool> {
    if !stamp
        && refresh == Refresh::Latest
        && archived.payload_hash.as_deref() == Some(payload_hash)
    {
        if parent.is_some() && archived.parent_visit_id.is_none() {
            archive.execute(
                "UPDATE visits SET parent_visit_id=?2 WHERE id=?1",
                params![archived.id, parent],
            )?;
            return Ok(true);
        }
        return Ok(false);
    }
    match refresh {
        Refresh::Latest => archive.execute(
            "UPDATE visits SET
               visit_duration_ms=CASE WHEN ?2 IS NOT NULL AND (visit_duration_ms IS NULL OR ?2>visit_duration_ms)
                 THEN ?2 ELSE visit_duration_ms END,
               is_known_to_sync=MAX(is_known_to_sync,?3), visited_link_id=COALESCE(visited_link_id,?4),
               payload_hash=?5, parent_visit_id=COALESCE(?6,parent_visit_id),
               source_visit_time=COALESCE(source_visit_time,?7)
             WHERE id=?1",
            params![
                archived.id,
                visit.visit_duration_ms,
                visit.is_known_to_sync as i64,
                visit.visited_link_id,
                payload_hash,
                parent,
                stamp.then(|| visit.source_visit_time.clone()).flatten(),
            ],
        )?,
        Refresh::SameCapture => archive.execute(
            "UPDATE visits SET
               visit_duration_ms=CASE WHEN ?2 IS NOT NULL AND (visit_duration_ms IS NULL OR ?2>visit_duration_ms)
                 THEN ?2 ELSE visit_duration_ms END,
               is_known_to_sync=MAX(is_known_to_sync,?3),
               parent_visit_id=COALESCE(parent_visit_id,?4)
             WHERE id=?1",
            params![
                archived.id,
                visit.visit_duration_ms,
                visit.is_known_to_sync as i64,
                parent,
            ],
        )?,
    };
    Ok(true)
}

/// Names the visit that opened `child`, once the read that found it is over: the opener came
/// later in the browser file than its child (Sync writes older visits after newer ones), or
/// before the rows this backup read.
pub(super) fn set_parent(archive: &Session<'_>, child: i64, parent: i64) -> Result<()> {
    archive.execute(
        "UPDATE visits SET parent_visit_id=?2 WHERE id=?1 AND parent_visit_id IS NULL",
        params![child, parent],
    )?;
    Ok(())
}

/// Saves one download: a new row, or the newest state of one the archive has. Chrome numbers a
/// new download one past the highest id, so a deleted newest download's id comes back for the
/// next one. A download is its guid, or, for the oldest rows without one, where it was saved and
/// when it started. Its state, sizes and paths change while it runs and after (a file moved or
/// removed); the newest read wins. Returns whether the download is new.
pub(super) fn upsert_download(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    download: &ParsedDownload,
    payload_hash: &str,
) -> Result<u64> {
    let stored = match &download.guid {
        Some(guid) if !guid.is_empty() => archive.query(
            "SELECT id,payload_hash FROM downloads INDEXED BY idx_downloads_profile_guid
             WHERE source_profile_id=?1 AND guid=?2 ORDER BY id LIMIT 1",
            params![source_profile_id, guid.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<Option<String>>(1)?)),
        )?,
        _ => archive.query(
            "SELECT id,payload_hash FROM downloads
             WHERE source_profile_id=?1 AND target_path IS ?2 AND start_time_ms IS ?3
               AND (guid IS NULL OR guid='') ORDER BY id LIMIT 1",
            params![source_profile_id, download.target_path.clone(), download.start_time_ms],
            |r| Ok((r.get::<i64>(0)?, r.get::<Option<String>>(1)?)),
        )?,
    }
    .pop();
    if let Some((id, stored_hash)) = stored {
        if stored_hash.as_deref() != Some(payload_hash) {
            archive.execute(
                "UPDATE downloads SET source_download_id=?2, current_path=?3, target_path=?4,
                   received_bytes=?5, total_bytes=?6, state=?7, mime_type=?8,
                   original_mime_type=?9, payload_hash=?10, recorded_at=?11
                 WHERE id=?1",
                params![
                    id,
                    download.source_download_id.to_string(),
                    download.current_path.clone(),
                    download.target_path.clone(),
                    download.received_bytes,
                    download.total_bytes,
                    download.state,
                    download.mime_type.clone(),
                    download.original_mime_type.clone(),
                    payload_hash,
                    now_rfc3339(),
                ],
            )?;
        }
        return Ok(0);
    }
    archive.execute(
        "INSERT INTO downloads (
           source_download_id,
           guid,
           current_path,
           target_path,
           start_time_ms,
           start_time_iso,
           total_bytes,
           received_bytes,
           state,
           mime_type,
           original_mime_type,
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            download.source_download_id.to_string(),
            download.guid.clone(),
            download.current_path.clone(),
            download.target_path.clone(),
            download.start_time_ms,
            download.start_time_iso.clone(),
            download.total_bytes,
            download.received_bytes,
            download.state,
            download.mime_type.clone(),
            download.original_mime_type.clone(),
            source_profile_id,
            run_id,
            payload_hash,
            now_rfc3339(),
        ],
    )
}

/// Inserts one normalized search-term row linked to the canonical URL it came from.
pub(super) fn insert_search_term(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile_id: &str,
    url_id: i64,
    term: &ParsedSearchTerm,
) -> Result<u64> {
    archive.execute(
        "INSERT OR IGNORE INTO search_terms (
               url_id,
               term,
               normalized_term,
               source_profile_id,
               created_by_run_id,
               profile_id,
               keyword_id,
               recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            url_id,
            term.term.clone(),
            term.normalized_term.clone(),
            source_profile_id,
            run_id,
            profile_id,
            term.keyword_id,
            now_rfc3339(),
        ],
    )
}

/// Inserts one favicon payload row for later Explorer/detail rendering.
pub(super) fn insert_favicon(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    favicon: &ParsedFavicon,
    payload_hash: &str,
) -> Result<u64> {
    let image_blob_hash = favicon.image_data.as_deref().map(sha256_hex);
    let metadata = favicon_url_metadata(&favicon.page_url);
    if let (Some(blob_hash), Some(image_data)) =
        (image_blob_hash.as_deref(), favicon.image_data.as_deref())
    {
        archive.execute(
            "INSERT OR IGNORE INTO favicon_blobs (
               blob_hash,
               image_data,
               recorded_at
             )
             VALUES (?1, ?2, ?3)",
            params![blob_hash, image_data, now_rfc3339(),],
        )?;
    }

    archive.execute(
        "INSERT OR IGNORE INTO favicons (
           page_url,
           icon_url,
           icon_type,
           width,
           height,
           last_updated_ms,
           last_updated_iso,
           image_data,
           image_blob_hash,
           page_host,
           page_registrable_domain,
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            favicon.page_url.clone(),
            favicon.icon_url.clone(),
            favicon.icon_type,
            favicon.width,
            favicon.height,
            favicon.last_updated_ms,
            favicon.last_updated_iso.clone(),
            Option::<Vec<u8>>::None,
            image_blob_hash,
            metadata.host,
            metadata.registrable_domain,
            source_profile_id,
            run_id,
            payload_hash,
            now_rfc3339(),
        ],
    )
}

/// Updates the stored first/last visit bounds for one canonical URL after visit inserts.
pub(super) fn sync_url_bounds(
    archive: &Session<'_>,
    url_id: i64,
    bounds: &UrlVisitBounds,
) -> Result<()> {
    archive.execute(
        "UPDATE urls
         SET first_visit_ms = CASE
               WHEN ?2 < first_visit_ms THEN ?2
               ELSE first_visit_ms
             END,
         first_visit_iso = CASE
               WHEN ?2 < first_visit_ms THEN ?3
               ELSE first_visit_iso
             END,
         last_visit_ms = CASE
               WHEN ?4 > last_visit_ms THEN ?4
               ELSE last_visit_ms
             END,
         last_visit_iso = CASE
               WHEN ?4 > last_visit_ms THEN ?5
               ELSE last_visit_iso
             END
         WHERE id = ?1",
        params![
            url_id,
            bounds.first_visit_ms,
            bounds.first_visit_iso.clone(),
            bounds.last_visit_ms,
            bounds.last_visit_iso.clone()
        ],
    )?;
    Ok(())
}

/// Tracks per-URL min/max visit times before a batched URL-bound update.
pub(super) fn track_url_visit_bounds(
    url_bounds: &mut HashMap<i64, UrlVisitBounds>,
    url_id: i64,
    visit: &ParsedVisit,
) {
    url_bounds
        .entry(url_id)
        .and_modify(|bounds| {
            if visit.visit_time_ms < bounds.first_visit_ms {
                bounds.first_visit_ms = visit.visit_time_ms;
                bounds.first_visit_iso = visit.visit_time_iso.clone();
            }
            if visit.visit_time_ms > bounds.last_visit_ms {
                bounds.last_visit_ms = visit.visit_time_ms;
                bounds.last_visit_iso = visit.visit_time_iso.clone();
            }
        })
        .or_insert_with(|| UrlVisitBounds {
            first_visit_ms: visit.visit_time_ms,
            first_visit_iso: visit.visit_time_iso.clone(),
            last_visit_ms: visit.visit_time_ms,
            last_visit_iso: visit.visit_time_iso.clone(),
        });
}
