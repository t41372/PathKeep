//! Responsibilities: maintain each page's searchable facts and durable handoff.
//! Not responsible for: index consumption, ranking or opening databases.
//! Failure modes: shared pages duplicate, reverted visits retain membership,
//! stale titles/annotations survive changes, enrichment disappears on refresh,
//! document and outbox commit separately. Tests: search_documents::tests.

use super::search_lexical::analyze_document;
use crate::utils::{now_rfc3339, url_domain};
use vault_store::{Result, Session, params};

#[cfg(test)]
mod tests;

/// Refreshes only the affected page; callers include this in their fact transaction.
pub(crate) fn refresh(s: &Session<'_>, page_id: i64) -> Result<()> {
    refresh_text(s, page_id)?;
    refresh_memberships(s, page_id)?;
    enqueue(s, page_id)
}

/// Metadata edits do not rescan all historical visits to an unchanged page.
pub(crate) fn text_changed(s: &Session<'_>, page_id: i64) -> Result<()> {
    refresh_text(s, page_id)?;
    enqueue(s, page_id)
}

/// Counts only newly inserted visits, so a frequently visited page stays linear
/// over the complete import rather than recounting its growing prefix each batch.
pub(crate) fn visits_added(
    s: &Session<'_>,
    page_id: i64,
    profile: i64,
    count: i64,
    newest: i64,
) -> Result<()> {
    refresh_text(s, page_id)?;
    s.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms)
        VALUES(?1,?2,?3,?4) ON CONFLICT(page_id,source_profile_id) DO UPDATE SET
        visible_visit_count=search_document_profiles.visible_visit_count+excluded.visible_visit_count,
        newest_visit_ms=MAX(search_document_profiles.newest_visit_ms,excluded.newest_visit_ms)",params![page_id,profile,count,newest])?;
    update_newest(s, page_id)?;
    enqueue(s, page_id)
}

fn refresh_text(s: &Session<'_>, page_id: i64) -> Result<()> {
    let url =
        s.query_one("SELECT url FROM pages WHERE id=?1", [page_id], |r| Ok(r.get::<String>(0)?))?;
    let title = s.query_one(
        "SELECT COALESCE((SELECT u.title FROM urls u WHERE u.page_id=?1
         AND EXISTS(SELECT 1 FROM visits v WHERE v.url_id=u.id AND v.reverted_at IS NULL)
         ORDER BY u.last_visit_ms DESC,u.id DESC LIMIT 1),'')",
        [page_id],
        |r| Ok(r.get::<String>(0)?),
    )?;
    let terms = s.query_one(
        "SELECT COALESCE(group_concat(t.term,' '),'') FROM urls u JOIN search_terms t
         ON t.source_profile_id=u.source_profile_id AND t.url_id=u.id
         WHERE u.page_id=?1 AND t.reverted_at IS NULL",
        [page_id],
        |r| Ok(r.get::<String>(0)?),
    )?;
    let (notes, tags) = s.query_one(
        "SELECT COALESCE((SELECT notes FROM url_annotations WHERE url=?1),''),
         COALESCE((SELECT group_concat(tag,' ') FROM (SELECT tag FROM url_tags WHERE url=?1 ORDER BY created_at,tag)),'')",
        [url.as_str()], |r| Ok((r.get::<String>(0)?,r.get::<String>(1)?)))?;
    let lexical = analyze_document(&url, &title, &terms, &notes, &tags);
    s.execute(
        "INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,
         normalized_title,normalized_search_terms,cjk_grams,compact_text,domain,updated_at,notes_text,tags_text)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(page_id) DO UPDATE SET url=excluded.url,title=excluded.title,
         search_terms=excluded.search_terms,normalized_url=excluded.normalized_url,
         normalized_title=excluded.normalized_title,normalized_search_terms=excluded.normalized_search_terms,
         cjk_grams=excluded.cjk_grams,compact_text=excluded.compact_text,domain=excluded.domain,
         updated_at=excluded.updated_at,notes_text=excluded.notes_text,tags_text=excluded.tags_text",
        params![page_id,url.as_str(),title,terms,lexical.normalized_url,lexical.normalized_title,
        lexical.normalized_search_terms,lexical.cjk_grams,lexical.compact_text,url_domain(&url),now_rfc3339(),notes,tags],
    )?;
    Ok(())
}

fn refresh_memberships(s: &Session<'_>, page_id: i64) -> Result<()> {
    s.execute("DELETE FROM search_document_profiles WHERE page_id=?1", [page_id])?;
    s.execute(
        "INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms)
         SELECT ?1,v.source_profile_id,COUNT(*),MAX(v.visit_time_ms)
         FROM urls u JOIN visits v ON v.url_id=u.id
         WHERE u.page_id=?1 AND v.reverted_at IS NULL GROUP BY v.source_profile_id", [page_id],
    )?;
    update_newest(s, page_id)
}

fn update_newest(s: &Session<'_>, page_id: i64) -> Result<()> {
    s.execute("UPDATE search_documents SET newest_visit_ms=(SELECT MAX(newest_visit_ms) FROM search_document_profiles WHERE page_id=?1) WHERE page_id=?1",[page_id])?;
    Ok(())
}

/// Annotations may precede the first visit; their text joins the first document later.
pub(crate) fn annotations_changed(s: &Session<'_>, url: &str) -> Result<()> {
    let ids = s.query("SELECT id FROM pages WHERE url=?1", [url], |r| Ok(r.get::<i64>(0)?))?;
    for id in ids {
        text_changed(s, id)?;
    }
    Ok(())
}

/// Enrichment's bounded text belongs to this same document/outbox transaction.
pub(crate) fn set_enrichment(s: &Session<'_>, page_id: i64, text: &str) -> Result<()> {
    let changed = s.execute(
        "UPDATE search_documents SET enrichment_text=?2,updated_at=?3 WHERE page_id=?1",
        params![page_id, text, now_rfc3339()],
    )?;
    if changed > 0 {
        enqueue(s, page_id)?;
    }
    Ok(())
}

fn enqueue(s: &Session<'_>, page_id: i64) -> Result<()> {
    s.execute("INSERT INTO search_outbox(page_id,change) VALUES(?1,'upsert')", [page_id])?;
    Ok(())
}

/// Allows enrichment writers to publish their bounded search text atomically.
pub fn set_enrichment_text(vault: &crate::Vault, page_id: i64, text: &str) -> anyhow::Result<()> {
    let text = text.to_owned();
    Ok(vault.store().write(
        vault_store::Db::Archive,
        vault_store::Priority::Interactive,
        move |s| set_enrichment(s, page_id, &text),
    )?)
}
