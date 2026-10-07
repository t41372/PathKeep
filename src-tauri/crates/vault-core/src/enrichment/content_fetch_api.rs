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
    use crate::enrichment::rate_limit;
    use crate::models::{AppConfig, ContentFetchExtractorPreference};
    use crate::test_support::native::TestDatabase;

    fn fixture() -> Vault {
        let owner = Vault::open_temp().unwrap();
        let mut config = owner.config().clone();
        config.initialized = true;
        config.ai.content_fetch_enabled = true;
        owner.with_config(config)
    }
    fn request(url: &str) -> ContentFetchNowRequest {
        ContentFetchNowRequest {
            history_id: Some(1),
            profile_id: Some("chrome:Default".into()),
            url: url.into(),
            title: None,
        }
    }
    fn seed(vault: &Vault, id: i64, url: &str) {
        let url = url.to_owned();
        vault.write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','') ON CONFLICT(id) DO NOTHING",())?;
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?3,?4)",params![id,url,crate::pages::canonical_key(&url),crate::visit_taxonomy::registrable_domain_for_url(&url)])?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,1,'',1,'')",params![id,url])?;
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,1,'')",[id])?;
            s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url',?1,'')",[crate::pages::canonical_key(&url)])?;Ok(())
        }).unwrap();
    }
    fn unique_host(tag: &str) -> String {
        format!("drain-{tag}-{}.example", std::process::id())
    }

    #[test]
    fn enqueue_content_fetch_now_is_disabled_when_master_switch_off() {
        let vault = Vault::open_temp().unwrap();
        let result = enqueue_content_fetch_now(&vault, &request("https://github.com/o/r")).unwrap();
        assert_eq!(result.state, "disabled");
        assert_eq!(result.job_id, 0);
    }
    #[test]
    fn enqueue_content_fetch_now_queues_a_job_when_enabled() {
        let vault = fixture();
        let result = enqueue_content_fetch_now(&vault, &request("https://github.com/o/r")).unwrap();
        assert_eq!(result.state, "queued");
        assert!(result.job_id > 0);
    }
    #[test]
    fn content_fetch_settings_reports_consent_and_zero_jobs_initially() {
        let settings = content_fetch_settings(&fixture()).unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.queued_jobs, 0);
        assert_eq!(settings.stored_records, 0);
        assert_eq!(settings.extractors.len(), 2);
    }
    #[test]
    fn drain_one_content_fetch_job_is_noop_without_consent() {
        assert!(!drain_one_content_fetch_job(&Vault::open_temp().unwrap()).unwrap());
    }
    #[test]
    fn drain_one_content_fetch_job_picks_up_a_due_job_when_enabled() {
        let vault = fixture();
        assert!(!drain_one_content_fetch_job(&vault).unwrap());
        let host = unique_host("due");
        let url = format!("https://{host}/post");
        seed(&vault, 1, &url);
        rate_limit::reset_host_bucket_for_test(&host);
        enqueue_content_fetch_now(&vault, &request(&url)).unwrap();
        assert!(drain_one_content_fetch_job(&vault).unwrap());
        assert_eq!(list_visit_enrichment(&vault, 1).unwrap().len(), 1);
    }
    #[test]
    fn drain_one_content_fetch_job_returns_false_for_a_rate_limited_job() {
        let vault = fixture();
        let host = unique_host("rate-limited");
        let url = format!("https://{host}/post");
        seed(&vault, 1, &url);
        while rate_limit::acquire_host_token(&host) {}
        enqueue_content_fetch_now(&vault, &request(&url)).unwrap();
        assert!(!drain_one_content_fetch_job(&vault).unwrap());
        assert_eq!(content_fetch_settings(&vault).unwrap().queued_jobs, 1);
        assert!(content_fetch_schedule_eta_secs(&vault).unwrap().unwrap() > 0);
        rate_limit::reset_host_bucket_for_test(&host);
    }
    #[test]
    fn list_visit_enrichment_is_empty_for_unknown_history_id() {
        assert!(list_visit_enrichment(&fixture(), 999).unwrap().is_empty());
    }
    #[test]
    fn enqueue_working_set_is_noop_without_consent() {
        assert_eq!(
            enqueue_content_fetch_working_set(&Vault::open_temp().unwrap(), 100).unwrap(),
            0
        );
    }
    #[test]
    fn enqueue_working_set_enqueues_a_starred_due_url() {
        let vault = fixture();
        seed(&vault, 1, "https://github.com/o/r");
        assert_eq!(enqueue_content_fetch_working_set(&vault, 100).unwrap(), 1);
        enqueue_content_fetch_working_set(&vault, 100).unwrap();
        assert_eq!(content_fetch_settings(&vault).unwrap().queued_jobs, 1);
    }
    #[test]
    fn enqueue_working_set_resolves_a_tracking_param_raw_url() {
        let vault = fixture();
        seed(&vault, 1, "https://github.com/o/r?utm_source=newsletter");
        assert_eq!(enqueue_content_fetch_working_set(&vault, 100).unwrap(), 1);
        let json = vault
            .read(Db::Intelligence, |s| {
                Ok(s.query_one("SELECT payload_json FROM intelligence_jobs", (), |r| {
                    Ok(r.get::<String>(0)?)
                })?)
            })
            .unwrap();
        let payload: EnrichmentJobPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload.history_id, Some(1));
        assert_eq!(payload.url, "https://github.com/o/r");
    }
    #[test]
    fn enqueue_working_set_skips_blocked_and_undue_candidates() {
        let vault = fixture();
        seed(&vault, 1, "https://github.com/blocked-co/r");
        seed(&vault, 2, "https://github.com/fresh-co/r");
        let mut config = vault.config().clone();
        config.ai.content_fetch_domains = vec![crate::models::ContentFetchDomainRule {
            domain: "github.com".into(),
            allowed: false,
        }];
        assert_eq!(enqueue_content_fetch_working_set(&vault.with_config(config), 100).unwrap(), 0);
        TestDatabase::new(&vault,Db::Intelligence).execute("INSERT INTO page_content_enrichments(canonical_url,content_source,fetch_status,fetched_at,snippet_json,extraction_json,pipeline_version,extractor_version,enrichment_summary) VALUES('https://github.com/fresh-co/r','github-repo','success','2026-06-21T00:00:00Z','[]','{}','v1',1,'s')",()).unwrap();
        assert_eq!(enqueue_content_fetch_working_set(&vault, 100).unwrap(), 1);
    }
    #[test]
    fn enqueue_working_set_skips_extractor_disabled_and_unarchived_candidates() {
        let vault = fixture();
        seed(&vault, 1, "https://github.com/a/r");
        TestDatabase::new(&vault,Db::Archive).execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url','https://no-page.example/page','')",()).unwrap();
        let mut config = vault.config().clone();
        config.ai.content_fetch_extractors = vec![ContentFetchExtractorPreference {
            extractor_id: "github-repo".into(),
            enabled: false,
        }];
        assert_eq!(enqueue_content_fetch_working_set(&vault.with_config(config), 100).unwrap(), 0);
    }
    #[test]
    fn list_visit_enrichment_returns_stored_rows() {
        let vault = fixture();
        seed(&vault, 42, "https://github.com/o/r");
        TestDatabase::new(&vault,Db::Intelligence).execute("INSERT INTO page_content_enrichments(canonical_url,content_source,fetch_status,fetched_at,readable_title,snippet_json,extraction_json,pipeline_version,extractor_version,enrichment_summary,final_url,http_status,refetch_after) VALUES('https://github.com/o/r','github-repo','success','2026-06-21T00:00:00Z','o/r','[]','{\"topics\":[\"wasm\"]}','v1',1,'A repo','https://github.com/o/r',200,NULL)",()).unwrap();
        let records = list_visit_enrichment(&vault, 42).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content_source, "github-repo");
        assert_eq!(records[0].summary.as_deref(), Some("A repo"));
        assert_eq!(records[0].readable_title.as_deref(), Some("o/r"));
        assert_eq!(records[0].http_status, Some(200));
    }
    #[test]
    fn content_fetch_schedule_eta_is_none_without_consent_and_reports_deferral() {
        let vault = fixture();
        assert_eq!(
            content_fetch_schedule_eta_secs(&vault.with_config(AppConfig::default())).unwrap(),
            None
        );
        let job = enqueue_content_fetch_now(&vault, &request("https://github.com/o/r")).unwrap();
        let future = (chrono::Utc::now() + chrono::Duration::seconds(20))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        TestDatabase::new(&vault, Db::Intelligence)
            .execute(
                "UPDATE intelligence_jobs SET scheduled_at=?1 WHERE id=?2",
                params![future, job.job_id],
            )
            .unwrap();
        let eta = content_fetch_schedule_eta_secs(&vault).unwrap().unwrap();
        assert!((1..=21).contains(&eta));
    }
    #[test]
    fn candidate_clock_is_in_unix_epoch_frame() {
        let before = chrono::Utc::now().timestamp_millis();
        let now = candidate_now_ms();
        assert!((now - before).abs() < 1000);
    }
}
