//! Responsibilities: compare native cached counters, references and the outbox with facts.
//! Not responsible for: repairing files, Tantivy consumption or intelligence state.
//! Failure modes: stale totals, missing search documents, invalid latest outbox events,
//! orphan references hidden by a successful structural check. Tests: doctor_tests.
use vault_store::{Result, Session};

pub(super) const COUNTERS: &[(&str, &str)] = &[
    ("sourceProfiles", "SELECT COUNT(*) FROM source_profiles WHERE enabled=1"),
    ("urls", "SELECT COUNT(*) FROM urls"),
    ("visits", "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL"),
    ("downloads", "SELECT COUNT(*) FROM downloads WHERE reverted_at IS NULL"),
    ("manifests", "SELECT COUNT(*) FROM manifests"),
    ("snapshots", "SELECT COUNT(*) FROM snapshots"),
];
pub(super) fn incorrect_totals(s: &Session<'_>) -> Result<Vec<(String, i64)>> {
    let cached = super::totals::load_cached_archive_totals(s)?;
    let mut incorrect = Vec::new();
    for &(name, sql) in COUNTERS {
        let actual = s.query_one(sql, (), |r| Ok(r.get::<i64>(0)?))?;
        if cached.get(name) != Some(&actual) {
            incorrect.push((name.to_owned(), actual));
        }
    }
    s.for_each("SELECT p.id,COALESCE((SELECT value FROM archive_totals WHERE name='sourceVisits:'||p.id),0),
        (SELECT COUNT(*) FROM visits INDEXED BY idx_visits_visible_profile_time_id WHERE source_profile_id=p.id AND reverted_at IS NULL) FROM source_profiles p",(),|r| {
        let (profile,stored,actual)=(r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?);
        if stored!=actual {incorrect.push((format!("sourceVisits:{profile}"),actual));}
        Ok(())
    })?;
    Ok(incorrect)
}
pub(super) const BROKEN_VISIBILITY: &str = "reverted_at IS NOT NULL AND (reverted_by_run_id IS NULL OR NOT EXISTS(SELECT 1 FROM runs WHERE runs.id=visits.reverted_by_run_id))";
pub(super) const BROKEN_OUTBOX: &str = "NOT EXISTS(SELECT 1 FROM search_outbox newer WHERE newer.page_id=search_outbox.page_id AND newer.seq>search_outbox.seq) AND (change NOT IN ('upsert','delete') OR (change='upsert' AND NOT EXISTS(SELECT 1 FROM search_documents WHERE page_id=search_outbox.page_id)) OR (change='delete' AND EXISTS(SELECT 1 FROM search_documents WHERE page_id=search_outbox.page_id)))";
pub(super) const DAMAGED_PAGES: &str="NOT EXISTS(SELECT 1 FROM search_documents WHERE page_id=pages.id)
 OR EXISTS(SELECT 1 FROM search_document_profiles p WHERE p.page_id=pages.id AND
 (p.visible_visit_count!=(SELECT COUNT(*) FROM urls u CROSS JOIN visits v ON v.url_id=u.id WHERE u.page_id=pages.id AND v.source_profile_id=p.source_profile_id AND v.reverted_at IS NULL)
 OR p.newest_visit_ms!=(SELECT MAX(v.visit_time_ms) FROM urls u CROSS JOIN visits v ON v.url_id=u.id WHERE u.page_id=pages.id AND v.source_profile_id=p.source_profile_id AND v.reverted_at IS NULL)))
 OR EXISTS(SELECT 1 FROM urls u WHERE u.page_id=pages.id AND EXISTS(SELECT 1 FROM visits v WHERE v.url_id=u.id AND v.reverted_at IS NULL)
 AND NOT EXISTS(SELECT 1 FROM search_document_profiles p WHERE p.page_id=pages.id AND p.source_profile_id=u.source_profile_id))";
pub(super) const BAD_BATCH_TOTAL: &str = "visible_items!=(SELECT COUNT(*) FROM visits WHERE import_batch_id=import_batches.id AND reverted_at IS NULL)";
pub(super) fn incorrect_batch_count(s: &Session<'_>) -> Result<i64> {
    s.query_one(&format!("SELECT COUNT(*) FROM import_batches WHERE {BAD_BATCH_TOTAL}"), (), |r| {
        Ok(r.get(0)?)
    })
}
pub(super) fn outbox_damage(s: &Session<'_>) -> Result<i64> {
    let documents =
        s.query_one(&format!("SELECT COUNT(*) FROM pages WHERE {DAMAGED_PAGES}"), (), |r| {
            Ok(r.get::<i64>(0)?)
        })?;
    let events = s.query_one(
        &format!("SELECT COUNT(*) FROM search_outbox WHERE {BROKEN_OUTBOX}"),
        (),
        |r| Ok(r.get::<i64>(0)?),
    )?;
    Ok(documents + events)
}
/// Turso 0.8.1 has no foreign_key_check pragma; check canonical relationships explicitly.
pub(super) fn orphan_facts(s: &Session<'_>) -> Result<i64> {
    let mut count = 0;
    for (table, column, parent) in [
        ("urls", "page_id", "pages"),
        ("urls", "source_profile_id", "source_profiles"),
        ("urls", "created_by_run_id", "runs"),
        ("visits", "url_id", "urls"),
        ("visits", "source_profile_id", "source_profiles"),
        ("visits", "created_by_run_id", "runs"),
        ("downloads", "source_profile_id", "source_profiles"),
        ("downloads", "created_by_run_id", "runs"),
        ("search_terms", "url_id", "urls"),
        ("search_terms", "source_profile_id", "source_profiles"),
        ("favicons", "source_profile_id", "source_profiles"),
        ("favicons", "image_blob_hash", "favicon_blobs"),
        ("og_images", "image_blob_hash", "og_image_blobs"),
        ("manifests", "run_id", "runs"),
        ("snapshots", "run_id", "runs"),
    ] {
        let key = if parent.ends_with("_blobs") { "blob_hash" } else { "id" };
        count+=s.query_one(&format!("SELECT COUNT(*) FROM {table} WHERE {column} IS NOT NULL AND NOT EXISTS(SELECT 1 FROM {parent} p WHERE p.{key}={table}.{column})"),(),|r|Ok(r.get::<i64>(0)?))?;
    }
    Ok(count)
}
