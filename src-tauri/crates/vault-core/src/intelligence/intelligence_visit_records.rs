//! Responsibilities: decode archive rows and apply the shared site-dictionary result.
//! Not responsible for: database ownership, pagination or persistence.
//! Failure modes: optional source IDs fail to decode, classifications lose provenance,
//! source pages omit visits, or search terms disappear at a batch boundary.
use super::{
    VisitRecord,
    site_dictionary::{SiteDictionaryEntry, normalize_query},
};
use vault_store::Row;

pub(super) fn visit_from_row(row: &Row) -> vault_store::Result<VisitRecord> {
    Ok(VisitRecord {
        visit_id: row.get(0)?,
        profile_id: row.get(1)?,
        source_profile_id: row.get(2)?,
        source_visit_id: row.get::<Option<i64>>(3)?.unwrap_or_default(),
        source_url_id: row.get(4)?,
        url: row.get(5)?,
        title: row.get(6)?,
        visit_time_ms: row.get(7)?,
        from_visit: row.get(8)?,
        transition_type: row.get(9)?,
        external_referrer_url: row.get(10)?,
        canonical_url: String::new(),
        registrable_domain: String::new(),
        domain_category: "unknown".to_string(),
        page_category: "unknown".to_string(),
        search_engine: None,
        search_query: None,
        is_new_domain: false,
        is_search_event: false,
        evidence_tier: "tier-c".to_string(),
        taxonomy_source: "unknown".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        display_name: None,
        session_id: None,
        trail_id: None,
    })
}

/// Loads the set of already-known domains for one profile so incremental
/// rebuilds can preserve first-seen semantics across batches.
pub(super) fn apply_site_dictionary(
    visit: &mut VisitRecord,
    query: Option<String>,
    dictionary: SiteDictionaryEntry,
) {
    visit.canonical_url = dictionary.canonical_url;
    visit.registrable_domain = dictionary.registrable_domain;
    visit.domain_category = dictionary.domain_category;
    visit.page_category = dictionary.page_category;
    visit.search_engine = dictionary.search_engine;
    visit.search_query = query
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_query(&value))
        .or(dictionary.search_query);
    visit.is_search_event = visit.search_query.is_some() && visit.search_engine.is_some();
    visit.evidence_tier = dictionary.evidence_tier;
    visit.taxonomy_source = dictionary.taxonomy_source;
    visit.taxonomy_pack = dictionary.taxonomy_pack;
    visit.taxonomy_version = dictionary.taxonomy_version;
    visit.display_name = dictionary.display_name;
}

#[cfg(test)]
mod tests {
    use super::VisitRecord;

    use vault_store::{Session, params_owned as params};

    /// Inserts run id 1 so the URL/visit foreign keys on `created_by_run_id`
    /// resolve.
    fn seed_run(connection: &Session<'_>) {
        connection
            .execute(
                "INSERT INTO runs (
                    id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
                 ) VALUES (1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                (),
            )
            .expect("insert run");
    }

    /// Inserts one archive profile so visits can resolve `profile_key`.
    fn seed_profile(connection: &Session<'_>, id: i64, profile_key: &str) {
        connection
            .execute(
                "INSERT INTO source_profiles (
                    id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
                 ) VALUES (?1, 'chrome', '1', ?2, '/tmp/profile', '2026-04-14T00:00:00Z', 1, ?2, '2026-04-14T00:00:00Z')",
                params![id, profile_key],
            )
            .expect("insert profile");
    }

    /// Inserts one URL + visit pair (and an optional search term) for a profile.
    #[allow(clippy::too_many_arguments)]
    fn seed_visit(
        connection: &Session<'_>,
        source_profile_id: i64,
        visit_id: i64,
        url: &str,
        title: &str,
        visit_time_ms: i64,
        normalized_search_term: Option<&str>,
    ) {
        let url_id = visit_id + 100;
        connection
            .execute(
                "INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?3,?4)",
                params![
                    url_id,
                    url,
                    crate::pages::canonical_key(url),
                    crate::visit_taxonomy::registrable_domain_for_url(url)
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO urls (
                    id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                    source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at,page_id
                 ) VALUES (?1, ?2, ?3, 1, 0, ?4, '2026-04-14T00:00:00Z', ?4, '2026-04-14T00:00:00Z', ?5, 1, ?6, 0, ?7, '2026-04-14T00:00:00Z',?1)",
                params![url_id, url, title, visit_time_ms, source_profile_id, url_id + 1000, format!("hash-{visit_id}")],
            )
            .expect("insert url");
        connection
            .execute(
                "INSERT INTO visits (
                    id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                    source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
                 ) VALUES (?1, ?2, ?3, ?4, '2026-04-14T00:00:00Z', 1, 0, ?5, 1, NULL, 0, ?6, ?7, '2026-04-14T00:00:00Z')",
                params![visit_id, url_id, visit_id.to_string(), visit_time_ms, source_profile_id, format!("fingerprint-{visit_id}"), format!("visit-hash-{visit_id}")],
            )
            .expect("insert visit");
        if let Some(term) = normalized_search_term {
            connection
                .execute(
                    "INSERT INTO search_terms (
                        id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                     ) VALUES (?1, ?2, ?3, ?3, ?4, 1, ?5)",
                    params![
                        visit_id + 10_000,
                        url_id,
                        term,
                        source_profile_id,
                        format!("chrome:Profile{source_profile_id}"),
                    ],
                )
                .expect("insert search term");
        }
    }

    fn seeded_connection() -> crate::Vault {
        let vault = crate::Vault::open_temp().unwrap();
        vault.write(vault_store::Db::Archive,vault_store::Priority::Batch,|s| {
        seed_run(s);
        seed_profile(s, 1, "chrome:Profile1");
        seed_profile(s, 2, "chrome:Profile2");
        seed_visit(
            s,
            1,
            1,
            "https://www.google.com/search?q=sqlite+wal",
            "sqlite wal - Google Search",
            1_711_929_600_000,
            Some("sqlite wal"),
        );
        seed_visit(s, 1, 2, "https://example.com/two", "Two", 1_711_929_660_000, None);
        seed_visit(s, 1, 3, "https://example.com/three", "Three", 1_712_016_000_000, None);
        seed_visit(s, 2, 20, "https://work.example/a", "Work A", 1_712_000_000_000, None);
        seed_visit(s, 2, 21, "https://work.example/b", "Work B", 1_712_000_060_000, None);
        s.execute("UPDATE archive_totals SET value=5 WHERE name='visits'",())?;
        s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',3),('sourceVisits:2',2)",())?;
        Ok(())
        }).unwrap();
        vault
    }

    fn pages(vault: &crate::Vault, profile: Option<&str>, size: usize) -> Vec<VisitRecord> {
        let mut visits = Vec::new();
        for source in super::super::build_pipeline::sources(vault, profile).unwrap() {
            let state = super::super::build_state::BuildState {
                profile_id: source.profile_id,
                source_profile_id: source.source_id,
                source: source.watermark,
                ..Default::default()
            };
            let mut cursor = None;
            loop {
                let page =
                    super::super::build_rows::source_page(vault, &state, cursor, size).unwrap();
                if page.is_empty() {
                    break;
                }
                let last = page.last().unwrap();
                cursor = Some(super::super::VisibleVisitBatchCursor {
                    visit_time_ms: last.visit_time_ms,
                    visit_id: last.visit_id,
                });
                visits.extend(page);
            }
        }
        visits
    }

    fn visit_ids(visits: &[VisitRecord]) -> Vec<i64> {
        visits.iter().map(|visit| visit.visit_id).collect()
    }

    #[test]
    fn load_visible_visits_unbounded_returns_every_profile_in_ascending_order() {
        let vault = seeded_connection();

        let visits = pages(&vault, None, 4096);

        // Ascending by (visit_time_ms, id) across both profiles. Visit 3 is the
        // chronologically newest even though profile 2's ids are higher, proving
        // ordering keys on time rather than id.
        assert_eq!(visit_ids(&visits), vec![1, 2, 3, 20, 21]);
    }

    #[test]
    fn load_visible_visits_scopes_to_one_profile() {
        let vault = seeded_connection();

        let visits = pages(&vault, Some("chrome:Profile1"), 4096);

        assert_eq!(visit_ids(&visits), vec![1, 2, 3]);
        assert!(visits.iter().all(|visit| visit.profile_id == "chrome:Profile1"));
    }

    #[test]
    fn source_page_size_keeps_all_profile_visits_reachable() {
        let vault = seeded_connection();

        // Newest two of the scoped profile, still ascending after the DESC fetch.
        let limited = pages(&vault, Some("chrome:Profile1"), 2);
        assert_eq!(visit_ids(&limited), vec![1, 2, 3]);
    }

    #[test]
    fn source_pages_cover_every_profile() {
        let vault = seeded_connection();

        // No profile filter: the newest two rows globally by time are visit 21
        // (1_712_000_060_000) and visit 3 (1_712_016_000_000), returned
        // ascending after the DESC-limited fetch.
        let limited = pages(&vault, None, 2);
        assert_eq!(visit_ids(&limited), vec![1, 2, 3, 20, 21]);
    }

    #[test]
    fn zero_rebuild_page_size_still_covers_all_visits() {
        let vault = seeded_connection();

        crate::run_core_intelligence(
            &vault,
            &crate::CoreIntelligenceRebuildRequest { limit: Some(0), ..Default::default() },
        )
        .unwrap();
        let count =
            vault
                .store()
                .read(vault_store::Db::Intelligence, |s| {
                    s.query_one("SELECT COUNT(*) FROM visit_derived_facts", (), |r| {
                        Ok(r.get::<i64>(0)?)
                    })
                })
                .unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn oversized_source_page_returns_all_rows() {
        let vault = seeded_connection();

        let limited = pages(&vault, Some("chrome:Profile1"), 999);
        assert_eq!(visit_ids(&limited), vec![1, 2, 3]);
    }

    #[test]
    fn source_pages_hydrate_search_terms() {
        let vault = seeded_connection();

        // Visit 1 is the Google search; it must classify as a search event when
        // it falls inside the bounded window, proving hydration runs on the
        // trimmed-in-SQL set rather than the full table.
        let limited = pages(&vault, Some("chrome:Profile1"), 3);
        let search_visit =
            limited.iter().find(|visit| visit.visit_id == 1).expect("search visit present");
        assert_eq!(search_visit.search_query.as_deref(), Some("sqlite wal"));
        crate::run_core_intelligence(&vault, &Default::default()).unwrap();
        let classified = vault
            .store()
            .read(vault_store::Db::Intelligence, |s| {
                s.query_one(
                    "SELECT is_search_event FROM visit_derived_facts WHERE visit_id=1",
                    (),
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap();
        assert_eq!(classified, 1);
    }
}
