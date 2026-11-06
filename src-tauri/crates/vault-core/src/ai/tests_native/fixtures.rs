//! Responsibilities: seed native AI data with complete indexes recorded as ready.
//! Not responsible for: implementing indexing or emulating the retired connection API.
//! Failure modes: fixture pages lack identities, counters omit visits, search documents
//! miss a page, enrichment text skips the search plane, or embedding totals drift.
use super::base_config;
use crate::ai::AiProviderRuntime;
use crate::ai::indexing::{IndexedVisit, upsert_embedding};
use crate::archive::{search_documents, totals};
use crate::config::ProjectPaths;
use crate::models::{AppConfig, ArchiveMode};
use crate::test_support::native::TestDatabase;
use crate::utils::now_rfc3339;
use anyhow::Result;
use browser_history_parser::chromium::chrome_time_to_unix_ms;
use std::collections::{BTreeMap, BTreeSet};
use vault_store::{Db, Priority, params_owned as params};

/// One history visit as an import leaves it. `urls.id`, `visits.id` and the source ids all
/// equal `history_id`, so tests can assert on the ids search returns.
pub(in crate::ai) struct SeedVisit {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub visit_ms: i64,
    pub visit_iso: String,
}

impl SeedVisit {
    /// Takes a Chromium microsecond timestamp, converted the way the browser readers do.
    pub(in crate::ai) fn chrome(
        history_id: i64,
        profile_id: &str,
        url: &str,
        title: Option<&str>,
        visit_time: i64,
    ) -> Self {
        Self {
            history_id,
            profile_id: profile_id.to_owned(),
            url: url.to_owned(),
            title: title.map(str::to_owned),
            visit_ms: chrome_time_to_unix_ms(visit_time),
            visit_iso: crate::utils::chrome_time_to_rfc3339(visit_time),
        }
    }

    /// Takes Unix milliseconds as given. The Chromium conversion clamps far-future values,
    /// which would give distinct test timestamps the same stored time.
    pub(in crate::ai) fn unix_ms(
        history_id: i64,
        profile_id: &str,
        url: &str,
        title: Option<&str>,
        visit_ms: i64,
    ) -> Self {
        let visit_iso = chrono::DateTime::from_timestamp_millis(visit_ms)
            .expect("fixture timestamp is in chrono's range")
            .to_rfc3339();
        Self {
            history_id,
            profile_id: profile_id.to_owned(),
            url: url.to_owned(),
            title: title.map(str::to_owned),
            visit_ms,
            visit_iso,
        }
    }
}

/// Fixture profiles get a stable row id derived from their key, so a test can name a
/// profile without first looking up its id.
pub(in crate::ai) fn profile_row_id(profile_id: &str) -> i64 {
    profile_id.bytes().fold(0_i64, |acc, value| acc + value as i64).max(1)
}

/// Writes every visit, its page, the archive counters and the search documents in one
/// archive transaction, then lets the search indexer consume the outbox once.
pub(in crate::ai) fn seed_visits(vault: &crate::Vault, visits: Vec<SeedVisit>) {
    vault
        .write(Db::Archive, Priority::Batch, move |s| {
            let now = now_rfc3339();
            s.execute(
                "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'test', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
                [now.as_str()],
            )?;
            let mut new_urls = 0;
            let mut visits_by_profile = BTreeMap::<i64, i64>::new();
            let mut pages = BTreeSet::new();
            for visit in visits {
                let profile = profile_row_id(&visit.profile_id);
                let browser_kind = visit.profile_id.split(':').next().unwrap_or("legacy");
                let new_profile = s.execute(
                    "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                     VALUES (?1, ?2, 'test', ?3, ?4, ?5, 1, ?3, ?5)",
                    params![
                        profile,
                        browser_kind,
                        visit.profile_id.as_str(),
                        format!("/tmp/{}", visit.profile_id),
                        now.as_str(),
                    ],
                )?;
                if new_profile > 0 {
                    totals::add(s, "sourceProfiles", 1)?;
                }
                let page_id = s.query_one(
                    "INSERT INTO pages(url,canonical_url,registrable_domain) VALUES(?1,?2,?3)
                     ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",
                    params![
                        visit.url.as_str(),
                        crate::pages::canonical_key(&visit.url),
                        crate::visit_taxonomy::registrable_domain_for_url(&visit.url),
                    ],
                    |r| Ok(r.get::<i64>(0)?),
                )?;
                new_urls += s.execute(
                    "INSERT OR IGNORE INTO urls
                     (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at, page_id)
                     VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?8, ?9)",
                    params![
                        visit.history_id,
                        visit.url.as_str(),
                        visit.title.as_deref(),
                        visit.visit_ms,
                        visit.visit_iso.as_str(),
                        profile,
                        format!("payload-{}", visit.history_id),
                        now.as_str(),
                        page_id,
                    ],
                )? as i64;
                s.execute(
                    "INSERT INTO visits
                     (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
                     VALUES (?1, ?1, ?2, ?3, ?4, 805306368, 0, ?5, 1, NULL, 1, 0, NULL, NULL, ?6, ?7, ?8)",
                    params![
                        visit.history_id,
                        visit.history_id.to_string(),
                        visit.visit_ms,
                        visit.visit_iso.as_str(),
                        profile,
                        format!("fp-{}", visit.history_id),
                        format!("payload-{}", visit.history_id),
                        now.as_str(),
                    ],
                )?;
                *visits_by_profile.entry(profile).or_default() += 1;
                pages.insert(page_id);
            }
            totals::add(s, "urls", new_urls)?;
            for (profile, count) in visits_by_profile {
                totals::add_visits(s, profile, count)?;
            }
            for page_id in pages {
                search_documents::refresh(s, page_id)?;
            }
            Ok(())
        })
        .expect("seed visits");
    vault.search().cycle().expect("index seeded pages");
}

pub(in crate::ai) fn seed_visit(
    db: &TestDatabase,
    history_id: i64,
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visit_time: i64,
) {
    seed_visits(&db.vault, vec![SeedVisit::chrome(history_id, profile_id, url, title, visit_time)]);
}

pub(in crate::ai) fn seed_successful_index_ledger(
    connection: &TestDatabase,
    provider: &AiProviderRuntime,
    source_watermark: i64,
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
             VALUES (?1, ?2, 'ai_embeddings', 'test-v1', 'ready', ?3, 1, ?4, ?4, ?4, NULL, NULL, NULL)",
            params![
                provider.config.id.as_str(),
                provider.config.default_model.as_str(),
                source_watermark,
                now_rfc3339(),
            ],
        )
        .expect("insert successful ledger");
}

/// A plaintext temp Vault with the AI test configuration. The returned database runs SQL
/// on the intelligence plane; `connection.vault` is the Vault the code under test reads.
pub(in crate::ai) fn prepared_archive() -> (ProjectPaths, AppConfig, TestDatabase) {
    prepared_archive_with_key(None)
}

/// Same as [`prepared_archive`], encrypted at rest when `key` is given.
pub(in crate::ai) fn prepared_archive_with_key(
    key: Option<[u8; 32]>,
) -> (ProjectPaths, AppConfig, TestDatabase) {
    let mut config = base_config();
    if key.is_some() {
        config.archive_mode = ArchiveMode::Encrypted;
    }
    let vault = crate::Vault::open_temp_with_key(key).unwrap().with_config(config.clone());
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
    db.write(move |s| upsert_embedding(s, &provider, &visit, &time, "legacy"))
}
pub(in crate::ai) fn embedding_count(
    db: &TestDatabase,
    provider: &str,
    model: &str,
) -> Result<i64> {
    let (provider, model) = (provider.to_owned(), model.to_owned());
    db.read(move |s| crate::ai::native_ledger::provider_embedding_count(s, &provider, &model))
}

/// Legacy fixtures deliberately write released flat files, then use the real startup migration.
pub(in crate::ai) fn migrate_fixture(vault: &crate::Vault, provider: &AiProviderRuntime) {
    let pair = provider.clone();
    vault.write(Db::Intelligence,Priority::Batch,move |s| {
        s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state) VALUES(?1,?2,'','test','ready') ON CONFLICT(provider_id,model) DO UPDATE SET sidecar_table=''",params![pair.config.id,pair.config.default_model])?;
        Ok(())
    }).unwrap();
    crate::ai::generations::migrate_flat_indexes(vault).unwrap();
}

/// Source assertions inspect a lineage explicitly; production readers only use serving leases.
pub(in crate::ai) fn source_paths(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
) -> ProjectPaths {
    if let Some(lease) =
        crate::ai::generations::resolve(vault, &provider.config.id, &provider.config.default_model)
            .unwrap()
    {
        let mut paths = vault.paths().clone();
        paths.vectors_dir = lease.paths().vectors_dir.parent().unwrap().to_owned();
        return paths;
    }
    let pair = provider.clone();
    let lineage = vault
        .read(Db::Intelligence, move |s| {
            Ok(s.query_one(
                "SELECT lineage FROM ai_embedding_totals WHERE provider_id=?1 AND model=?2 LIMIT 1",
                params![pair.config.id, pair.config.default_model],
                |r| Ok(r.get::<String>(0)?),
            )?)
        })
        .unwrap();
    let mut paths = vault.paths().clone();
    paths.vectors_dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join(lineage);
    paths
}
