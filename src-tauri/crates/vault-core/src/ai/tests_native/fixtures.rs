//! Responsibilities: seed native AI regression data and query the real metadata ledger.
//! Not responsible for: implementing indexing or emulating the retired connection API.
//! Failure modes: fixture pages lack identities, counters omit visits, or embedding totals drift.
use super::*;
pub(in crate::ai) fn seed_visit(
    db: &TestDatabase,
    history_id: i64,
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visit_time: i64,
) {
    let connection = TestDatabase::new(&db.vault, Db::Archive);
    let browser_kind = profile_id.split(':').next().unwrap_or("legacy");
    let profile_row_id = profile_id.bytes().fold(0_i64, |acc, value| acc + value as i64).max(1);
    connection
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert canonical run");
    connection
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (?1, ?2, 'test', ?3, ?4, ?5, 1, ?6, ?5)",
            params![
                profile_row_id,
                browser_kind,
                profile_id,
                format!("/tmp/{profile_id}"),
                now_rfc3339(),
                profile_id,
            ],
        )
        .expect("insert canonical profile");
    let page_url = url.to_owned();
    let page_id=connection.write(move |s|Ok(s.query_one("INSERT INTO pages(url,canonical_url,registrable_domain) VALUES(?1,?2,?3) ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",params![page_url,crate::pages::canonical_key(&page_url),crate::visit_taxonomy::registrable_domain_for_url(&page_url)],|r|Ok(r.get::<i64>(0)?))?)).unwrap();
    connection
        .execute(
            "INSERT OR IGNORE INTO urls
             (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at, page_id)
             VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?8, ?9)",
            params![
                history_id,
                url,
                title,
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time),
                profile_row_id,
                format!("payload-{history_id}"),
                now_rfc3339(),
                page_id
            ],
        )
        .expect("insert canonical url");
    connection
        .execute(
            "INSERT INTO visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 805306368, 0, ?6, 1, NULL, 1, 0, NULL, NULL, ?7, ?8, ?9)",
            params![
                history_id,
                history_id,
                history_id.to_string(),
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time),
                profile_row_id,
                format!("fp-{history_id}"),
                format!("payload-{history_id}"),
                now_rfc3339(),
            ],
        )
        .expect("insert visit");
    connection
        .execute("UPDATE archive_totals SET value=value+1 WHERE name IN ('visits','urls')", ())
        .unwrap();
    connection.execute("INSERT INTO archive_totals(name,value) VALUES(?1,1) ON CONFLICT(name) DO UPDATE SET value=value+1",[format!("sourceVisits:{profile_row_id}")]).unwrap();
    connection
        .write(move |session| {
            crate::archive::search_documents::refresh(session, page_id)?;
            Ok(())
        })
        .unwrap();
    db.vault.search().cycle().unwrap();
}

pub(in crate::ai) fn seed_failed_index_ledger(
    connection: &TestDatabase,
    provider: &AiProviderRuntime,
    failure_reason: &str,
) {
    connection
        .execute(
            "INSERT OR REPLACE INTO ai_index_ledger (
               provider_id,
               model,
               sidecar_table,
               index_version,
               state,
               source_watermark,
               last_run_id,
               build_started_at,
               build_finished_at,
               last_indexed_at,
               last_cleared_at,
               last_failure_at,
               failure_reason
             )
             VALUES (?1, ?2, 'ai_embeddings', 'test-v1', 'failed', NULL, NULL, NULL, NULL, NULL, NULL, ?3, ?4)",
            params![
                provider.config.id,
                provider.config.default_model,
                now_rfc3339(),
                failure_reason,
            ],
        )
        .expect("insert failed ledger");
}

pub(in crate::ai) fn prepared_archive() -> (ProjectPaths, AppConfig, TestDatabase) {
    let config = base_config();
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    (vault.paths().clone(), config, TestDatabase::new(&vault, Db::Intelligence))
}
pub(in crate::ai) fn seed_embedding(
    db: &TestDatabase,
    history_id: i64,
    provider: &AiProviderRuntime,
    hash: &str,
) {
    let visit = IndexedVisit {
        history_id,
        profile_id: "chrome:Default".into(),
        url: "https://example.com".into(),
        title: Some("Example".into()),
        domain: "example.com".into(),
        visited_at: "2026-04-04T00:00:00Z".into(),
        content: "content".into(),
        content_hash: hash.into(),
        content_key: 0,
        needs_embedding: true,
    };
    put_embedding(db, provider, &visit, &now_rfc3339()).unwrap();
}
pub(in crate::ai) fn put_embedding(
    db: &TestDatabase,
    provider: &AiProviderRuntime,
    visit: &IndexedVisit,
    time: &str,
) -> Result<bool> {
    let (provider, visit, time) = (provider.clone(), visit.clone(), time.to_owned());
    db.write(move |s| upsert_embedding(s, &provider, &visit, &time))
}
pub(in crate::ai) fn embedding_count(
    db: &TestDatabase,
    provider: &str,
    model: &str,
) -> Result<i64> {
    let (provider, model) = (provider.to_owned(), model.to_owned());
    db.read(move |s| crate::ai::native_ledger::provider_embedding_count(s, &provider, &model))
}

pub(in crate::ai) fn seed_extra_visit(
    db: &TestDatabase,
    visit_id: i64,
    url_id: i64,
    visit_time: i64,
) {
    let connection = TestDatabase::new(&db.vault, Db::Archive);
    let profile_row_id =
        "chrome:Default".bytes().fold(0_i64, |acc, value| acc + value as i64).max(1);
    connection
        .execute(
            "INSERT INTO visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 805306368, 0, ?6, 1, NULL, 1, 0, NULL, NULL, ?7, ?8, ?9)",
            params![
                visit_id,
                url_id,
                visit_id.to_string(),
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time),
                profile_row_id,
                format!("fp-{visit_id}"),
                format!("payload-{visit_id}"),
                now_rfc3339(),
            ],
        )
        .expect("insert extra visit");
    // Keep the url's last_visit bookkeeping consistent with the newer visit so the hydration picks the
    // most-recent visible visit as the representative.
    connection
        .execute(
            "UPDATE urls SET visit_count = visit_count + 1,
               last_visit_ms = MAX(last_visit_ms, ?2), last_visit_iso = ?3
             WHERE id = ?1",
            params![
                url_id,
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time)
            ],
        )
        .expect("bump url last_visit");
    connection
        .execute(
            "UPDATE archive_totals SET value=value+1 WHERE name='visits' OR name=?1",
            [format!("sourceVisits:{profile_row_id}")],
        )
        .unwrap();
    connection
        .write(move |session| {
            let page_id =
                session.query_one("SELECT page_id FROM urls WHERE id=?1", [url_id], |row| {
                    Ok(row.get::<i64>(0)?)
                })?;
            crate::archive::search_documents::refresh(session, page_id)?;
            Ok(())
        })
        .unwrap();
    db.vault.search().cycle().unwrap();
}
