//! Responsibilities: page through native candidates, enqueue fetches, and read stored outcomes.
//! Not responsible for: fetching, extraction or database ownership.
//! Failure modes: canonical variants disappear, a pool cap omits pages, absent pages never
//! become eligible, or a read mutates the queue. Native candidate and replay tests cover these.

use super::content_fetch::{content_fetch_allowed, execute_content_fetch_job_by_id};
use super::extractors::resolve_extractor;
use crate::Vault;
use crate::intelligence_runtime::{
    EnrichmentJobPayload, content_fetch_job_due, enqueue_content_fetch_job,
    next_content_fetch_schedule_eta_secs, next_queued_content_fetch_job,
};
use crate::models::{
    ContentFetchNowRequest, ContentFetchNowResult, ContentFetchSettings, VisitEnrichmentRecord,
};
use anyhow::Result;
use vault_store::{Db, OptionalRow, Priority, Session, params_owned as params};

/// Runs one due fetch outside the database writer.
pub fn drain_one_content_fetch_job(vault: &Vault) -> Result<bool> {
    if !vault.config().ai.content_fetch_enabled {
        return Ok(false);
    }
    reoffer_archived_candidates(vault)?;
    let Some(id) = next_queued_content_fetch_job(vault)? else {
        return Ok(false);
    };
    execute_content_fetch_job_by_id(vault, id)
}

/// Provides the worker's next wake-up without claiming or changing a job.
pub fn content_fetch_schedule_eta_secs(vault: &Vault) -> Result<Option<u64>> {
    if !vault.config().ai.content_fetch_enabled {
        return Ok(None);
    }
    next_content_fetch_schedule_eta_secs(vault)
}

fn candidate_now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Enumerates every eligible page; `page_size` bounds each transaction, never total coverage.
pub fn enqueue_content_fetch_working_set(vault: &Vault, page_size: usize) -> Result<usize> {
    if !vault.config().ai.content_fetch_enabled {
        return Ok(0);
    }
    let mut enqueued = reoffer_archived_candidates(vault)?;
    let mut cursor = None;
    let now = candidate_now_ms();
    loop {
        let page = crate::ai::select_working_set(
            vault,
            &crate::ai::WorkingSetConfig::default(),
            now,
            page_size.clamp(1, 512),
            cursor,
        )?;
        cursor = page.next_cursor;
        let allowed = page
            .items
            .into_iter()
            .filter(|item| content_fetch_allowed(vault.config(), &item.canonical_url))
            .collect::<Vec<_>>();
        let candidates = vault.read(Db::Archive, move |s| {
            let mut values = Vec::new();
            for item in allowed {
                if let Some(page) = resolve_page(s, &item.canonical_url)? {
                    let extractor =
                        resolve_extractor(&item.canonical_url).expect("allowed extractor");
                    values.push((
                        EnrichmentJobPayload {
                            history_id: page.history_id,
                            profile_id: page.profile_id,
                            url: item.canonical_url,
                            title: item.title,
                        },
                        extractor.id().to_owned(),
                        extractor.version() as i64,
                        -(item.score * 1000.0) as i64,
                    ));
                }
            }
            Ok(values)
        })?;
        enqueued+=vault.write(Db::Intelligence,Priority::Batch,move |s| {
            let mut count=0;
            for (payload,source,version,priority) in candidates {
                let fetch=content_fetch_job_due(s,&payload.url,&source,version)?;
                if !fetch {
                    let (summary,metadata)=s.query_one("SELECT enrichment_summary,extraction_json FROM page_content_enrichments WHERE canonical_url=?1 AND content_source=?2",params![payload.url,source],|r|Ok((r.get::<Option<String>>(0)?,r.get::<String>(1)?)))?;
                    let text=crate::archive::enrichment_text_for_index(summary.as_deref(),Some(&metadata));
                    let missing=s.query_one("SELECT EXISTS(SELECT 1 FROM archive.pages p CROSS JOIN archive.search_documents d ON d.page_id=p.id WHERE p.canonical_url=?1 AND d.enrichment_text<>?2 LIMIT 1 OFFSET 0)",params![payload.url,text],|r|Ok(r.get::<i64>(0)?!=0))?;
                    if !missing {continue;}
                }
                let id=enqueue_content_fetch_job(s,&payload)?;
                s.execute("UPDATE intelligence_jobs SET priority=?2 WHERE id=?1 AND state='queued'",params![id,priority])?;
                if !fetch {s.execute("UPDATE intelligence_jobs SET artifact_json=?2 WHERE id=?1 AND state='queued'",params![id,serde_json::json!({"contentStored":true,"contentSource":source}).to_string()])?;}
                count+=1;
            }Ok(count)
        })?;
        if cursor.is_none() {
            break;
        }
    }
    Ok(enqueued)
}

/// Manual fetch keeps its consent result explicit and its durable claim native.
pub fn enqueue_content_fetch_now(
    vault: &Vault,
    request: &ContentFetchNowRequest,
) -> Result<ContentFetchNowResult> {
    if !content_fetch_allowed(vault.config(), &request.url) {
        return Ok(ContentFetchNowResult {
            job_id: 0,
            state: "disabled".into(),
            note: "content_fetch.disabled_for_url".into(),
        });
    }
    let key = crate::pages::canonical_key(&request.url);
    let page = vault.read(Db::Archive, move |s| resolve_page(s, &key))?;
    let payload = EnrichmentJobPayload {
        history_id: page.as_ref().and_then(|p| p.history_id),
        profile_id: page.and_then(|p| p.profile_id),
        url: request.url.clone(),
        title: request.title.clone(),
    };
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        let job_id = enqueue_content_fetch_job(s, &payload)?;
        let state =
            s.query_one("SELECT state FROM intelligence_jobs WHERE id=?1", [job_id], |r| {
                Ok(r.get::<String>(0)?)
            })?;
        Ok(ContentFetchNowResult { job_id, state, note: "content_fetch.queued".into() })
    })
}

/// Each source contributes at most one row per visit; all sources are returned.
pub fn list_visit_enrichment(vault: &Vault, history_id: i64) -> Result<Vec<VisitEnrichmentRecord>> {
    let key=vault.read(Db::Archive,move |s|Ok(s.query_one("SELECT p.canonical_url FROM visits v JOIN urls u ON u.id=v.url_id JOIN pages p ON p.id=u.page_id WHERE v.id=?1",[history_id],|r|Ok(r.get::<String>(0)?)).optional()?))?;
    let Some(key) = key else {
        return Ok(Vec::new());
    };
    vault.read(Db::Intelligence, move |connection| {
        let mut statement = connection.prepare(
            "SELECT content_source, fetch_status, fetched_at, readable_title, enrichment_summary,
                extractor_version, extraction_json, final_url, http_status, refetch_after
         FROM page_content_enrichments
         WHERE canonical_url = ?1
         ORDER BY fetched_at DESC",
        )?;
        let rows = statement.query_map(params![key], |row| {
            Ok(VisitEnrichmentRecord {
                content_source: row.get(0)?,
                fetch_status: row.get(1)?,
                fetched_at: row.get(2)?,
                readable_title: row.get(3)?,
                summary: row.get(4)?,
                extractor_version: row.get(5)?,
                metadata_json: row.get::<Option<String>>(6)?,
                final_url: row.get(7)?,
                http_status: row.get(8)?,
                refetch_after: row.get(9)?,
            })
        })?;
        rows.collect::<vault_store::Result<Vec<_>>>().map_err(Into::into)
    })
}

/// Reads persisted queue totals without a legacy connection or password path.
pub fn content_fetch_settings(vault: &Vault) -> Result<ContentFetchSettings> {
    let config = vault.config().clone();
    vault.read(Db::Intelligence,move |s| {
        let count=|state:&str|->Result<usize> {Ok(s.query_one("SELECT COUNT(*) FROM intelligence_jobs WHERE job_type='content-fetch' AND state=?1",[state],|r|Ok(r.get::<i64>(0)?))? as usize)};
        let stored=s.query_one("SELECT COUNT(*) FROM page_content_enrichments WHERE content_source IN ('github-repo','generic-readable')",(),|r|Ok(r.get::<i64>(0)?))? as usize;
        Ok(ContentFetchSettings {enabled:config.ai.content_fetch_enabled,extractors:config.ai.content_fetch_extractors,domains:config.ai.content_fetch_domains,queued_jobs:count("queued")?,running_jobs:count("running")?,failed_jobs:count("failed")?,stored_records:stored})
    })
}

struct PageProvenance {
    history_id: Option<i64>,
    profile_id: Option<String>,
}
fn resolve_page(s: &Session<'_>, key: &str) -> Result<Option<PageProvenance>> {
    let mut pages=s.prepare("SELECT u.id,sp.profile_key,u.last_visit_ms FROM pages p LEFT JOIN urls u ON u.page_id=p.id LEFT JOIN source_profiles sp ON sp.id=u.source_profile_id WHERE p.canonical_url=?1")?;
    let mut visits=s.prepare("SELECT id,visit_time_ms FROM visits INDEXED BY idx_visits_visible_url_time WHERE url_id=?1 AND reverted_at IS NULL ORDER BY visit_time_ms DESC,id DESC LIMIT 1")?;
    let mut found = false;
    let mut newest = None::<(i64, i64, Option<String>)>;
    let mut fallback = None::<(i64, i64, Option<String>)>;
    for row in pages.query_map([key], |r| {
        Ok((r.get::<Option<i64>>(0)?, r.get::<Option<String>>(1)?, r.get::<Option<i64>>(2)?))
    })? {
        found = true;
        let (Some(url), profile, time) = row? else {
            continue;
        };
        let rank = (time.unwrap_or(0), url);
        if fallback.as_ref().is_none_or(|old| rank > (old.0, old.1)) {
            fallback = Some((rank.0, rank.1, profile.clone()));
        }
        if let Some((id, time)) =
            visits.query_one([url], |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?))).optional()?
            && newest.as_ref().is_none_or(|old| (time, id) > (old.0, old.1))
        {
            newest = Some((time, id, profile));
        }
    }
    Ok(found.then(|| PageProvenance {
        history_id: newest.as_ref().map(|row| row.1),
        profile_id: newest.and_then(|row| row.2).or_else(|| fallback.and_then(|row| row.2)),
    }))
}

/// Reoffers parked jobs only after the canonical page exists, in bounded transactions.
pub fn reoffer_archived_candidates(vault: &Vault) -> Result<usize> {
    let mut after = 0;
    let mut count = 0;
    loop {
        let rows=vault.read(Db::Intelligence,move |s| Ok(s.query("SELECT id,canonical_url FROM intelligence_jobs WHERE job_type='content-fetch' AND state='awaiting_archive' AND id>?1 ORDER BY id LIMIT 256",[after],|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?)))?))?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().unwrap().0;
        let ready = vault.read(Db::Archive, move |s| {
            let mut ready = Vec::new();
            for (id, key) in rows {
                if let Some(page) = resolve_page(s, &key)? {
                    ready.push((id, page));
                }
            }
            Ok(ready)
        })?;
        count+=vault.write(Db::Intelligence,Priority::Batch,move |s| {
            let mut changed=0;
            for (id,page) in ready {
                let json=s.query_one("SELECT payload_json FROM intelligence_jobs WHERE id=?1",[id],|r|Ok(r.get::<String>(0)?))?;
                let mut payload:EnrichmentJobPayload=serde_json::from_str(&json)?;
                payload.history_id=page.history_id;payload.profile_id=page.profile_id;
                changed+=s.execute("UPDATE intelligence_jobs SET state='queued',payload_json=?2,scheduled_at=?3,updated_at=?3,finished_at=NULL WHERE id=?1 AND state='awaiting_archive'",params![id,serde_json::to_string(&payload)?,crate::utils::now_rfc3339()])? as usize;
            }
            Ok(changed)
        })?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_one_content_fetch_job_is_noop_without_consent() {
        assert!(!drain_one_content_fetch_job(&Vault::open_temp().unwrap()).unwrap());
    }
}
