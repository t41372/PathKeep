//! Responsibilities: seed a native archive with browsing history through the path a backup
//! takes (run, source profile, source batch, `ArchiveChunkConsumer`), so a test reads exactly
//! what a real backup leaves: pages, totals, search documents and the outbox included.
//! Not responsible for: parsing browser files, other databases (Insights, agent), or any old
//! storage format.
//! Failure modes this guards against: fixture rows that skip a counter, page anchor or search
//! document that production maintains; a test assuming archive ids equal the source ids it
//! chose (the ids are returned explicitly).
use crate::{
    Vault,
    archive::{
        SourceBatchInput,
        ingest::{ArchiveChunkConsumer, WriteContext, parser::Watermark, writes},
        upsert_source_batch, write_runs,
    },
    models::{BrowserProfile, BrowserRetentionBoundary},
};
use browser_history_parser::{
    CapabilitySnapshot, HistoryBatchConsumer, ParsedSearchTerm, ParsedUrl, ParsedVisit,
};
use std::collections::BTreeMap;
use vault_store::{CancelToken, Db, Priority};

/// History for one browser profile, written in one backup run by [`History::commit`].
pub struct History<'a> {
    vault: &'a Vault,
    profile: BrowserProfile,
    urls: Vec<ParsedUrl>,
    visits: Vec<ParsedVisit>,
    terms: Vec<ParsedSearchTerm>,
}

/// Archive ids of what was written, keyed by the source ids the test chose.
#[derive(Debug, Default)]
pub struct Seeded {
    pub run_id: i64,
    pub source_profile_id: i64,
    pub urls: BTreeMap<i64, i64>,
    pub visits: BTreeMap<i64, i64>,
}

impl<'a> History<'a> {
    /// `profile_key` is `<browser>:<profile>`, e.g. `chrome:Default` or `firefox:main`.
    pub fn new(vault: &'a Vault, profile_key: &str) -> Self {
        let (browser, name) = profile_key.split_once(':').unwrap_or((profile_key, "Default"));
        let (family, browser_name) = match browser {
            "firefox" => ("firefox", "Firefox"),
            "safari" => ("safari", "Safari"),
            "edge" => ("chromium", "Edge"),
            _ => ("chromium", "Chrome"),
        };
        let profile = BrowserProfile {
            profile_id: profile_key.to_owned(),
            profile_name: name.to_owned(),
            browser_family: family.to_owned(),
            browser_name: browser_name.to_owned(),
            user_name: None,
            profile_path: format!("/fixture/{profile_key}"),
            history_path: Some(format!("/fixture/{profile_key}/History")),
            favicons_path: None,
            history_exists: true,
            history_readable: true,
            access_issue: None,
            browser_version: Some("fixture".to_owned()),
            history_file_name: "History".to_owned(),
            history_bytes: 0,
            favicons_bytes: 0,
            supporting_bytes: 0,
            retention_boundary: BrowserRetentionBoundary::default(),
        };
        Self { vault, profile, urls: Vec::new(), visits: Vec::new(), terms: Vec::new() }
    }

    pub fn chrome(vault: &'a Vault) -> Self {
        Self::new(vault, "chrome:Default")
    }

    /// One page visited once: source URL and visit both get `source_id`.
    pub fn page(self, source_id: i64, url: &str, title: Option<&str>, visit_ms: i64) -> Self {
        self.url(source_id, url, title).visit(source_id, source_id, visit_ms)
    }

    /// A page without visits yet; add them with [`History::visit`].
    pub fn url(mut self, source_url_id: i64, url: &str, title: Option<&str>) -> Self {
        self.urls.push(ParsedUrl {
            source_url_id,
            url: url.to_owned(),
            title: title.map(str::to_owned),
            visit_count: 0,
            typed_count: 0,
            last_visit_ms: 0,
            last_visit_iso: String::new(),
            hidden: false,
            source_last_visit_marker: None,
        });
        self
    }

    /// Another visit to a URL added earlier with [`History::url`] or [`History::page`].
    pub fn visit(mut self, source_visit_id: i64, source_url_id: i64, visit_ms: i64) -> Self {
        let url = self
            .urls
            .iter_mut()
            .find(|url| url.source_url_id == source_url_id)
            .expect("visit of a URL the fixture added");
        let iso = iso(visit_ms);
        url.visit_count += 1;
        if visit_ms >= url.last_visit_ms {
            url.last_visit_ms = visit_ms;
            url.last_visit_iso = iso.clone();
        }
        let (address, title) = (url.url.clone(), url.title.clone());
        self.visits.push(ParsedVisit {
            source_visit_id,
            source_url_id,
            url: address,
            title,
            visit_time_ms: visit_ms,
            visit_time_iso: iso,
            from_visit: None,
            transition: Some(805306368),
            visit_duration_ms: Some(0),
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        });
        self
    }

    /// A search the browser recorded as leading to a URL added earlier.
    pub fn search_term(mut self, source_url_id: i64, term: &str) -> Self {
        self.terms.push(ParsedSearchTerm {
            url_last_visit_time: 0,
            keyword_id: 1,
            url_id: source_url_id,
            term: term.to_owned(),
            normalized_term: term.to_lowercase(),
        });
        self
    }

    /// Writes everything in one backup run, as a backup would, and indexes it for search.
    pub fn commit(self) -> Seeded {
        let Self { vault, profile, urls, visits, terms } = self;
        let run_id =
            write_runs::start(vault, "backup", "manual", std::slice::from_ref(&profile.profile_id))
                .expect("start run");
        let owned = profile.clone();
        let source_profile_id = vault
            .write(Db::Archive, Priority::Batch, move |s| {
                Ok(writes::upsert_source_profile(s, &owned)?)
            })
            .expect("source profile");
        let input = SourceBatchInput {
            source_profile_id,
            run_id: Some(run_id),
            source_kind: "local_db".into(),
            browser_version: profile.browser_version.clone(),
            schema_version_text: None,
            schema_version_int: None,
            schema_fingerprint: "fixture".into(),
            capability_snapshot: CapabilitySnapshot::default(),
            coverage_stats_json: "{}".into(),
            artifact_refs_json: None,
            notes_json: None,
        };
        let batch = vault
            .write(Db::SourceEvidence, Priority::Batch, move |s| upsert_source_batch(s, &input))
            .expect("source batch");
        let context = WriteContext {
            run_id,
            source_profile_id,
            profile_id: profile.profile_id.clone(),
            browser_family: profile.browser_family.clone(),
            import: None,
        };
        let cancel = CancelToken::new();
        let mut consumer =
            ArchiveChunkConsumer::new(vault, context, Watermark::default(), &cancel, None, batch);
        consumer.urls(urls).expect("urls");
        consumer.visits(visits).expect("visits");
        consumer.search_terms(terms).expect("search terms");
        consumer.finish().expect("finish ingest");
        write_runs::complete(
            vault,
            run_id,
            serde_json::json!({}),
            serde_json::json!([]),
            crate::utils::now_rfc3339(),
        )
        .expect("complete run");
        vault.search().cycle().expect("index");
        let (urls, visits) = vault
            .read(Db::Archive, move |s| {
                let urls = s.query(
                    "SELECT source_url_id, id FROM urls WHERE source_profile_id=?1",
                    [source_profile_id],
                    |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
                )?;
                let visits = s.query(
                    "SELECT CAST(source_visit_id AS INTEGER), id FROM visits WHERE source_profile_id=?1",
                    [source_profile_id],
                    |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
                )?;
                Ok((urls, visits))
            })
            .expect("read seeded ids");
        Seeded {
            run_id,
            source_profile_id,
            urls: urls.into_iter().collect(),
            visits: visits.into_iter().collect(),
        }
    }
}

fn iso(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().to_rfc3339()
}
