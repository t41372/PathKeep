//! Site Dictionary and deterministic URL/query normalization helpers.
//!
//! Core Intelligence uses one shared site-dictionary layer so session/trail/
//! rollup queries stay consistent about canonical URLs, registrable domains,
//! search-engine detection, and display aliases.

use crate::{
    deterministic::{
        DomainCategory, InteractionKind, PageCategory, TaxonomyOverride, TaxonomyOverrideTarget,
        VisitAnalysisInput, analyze_visit,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};

#[derive(Debug, Clone, Default)]
pub(crate) struct SiteDictionaryEntry {
    pub canonical_url: String,
    pub registrable_domain: String,
    pub domain_category: String,
    pub page_category: String,
    pub search_engine: Option<String>,
    pub search_query: Option<String>,
    pub evidence_tier: String,
    pub taxonomy_source: String,
    pub taxonomy_pack: Option<String>,
    pub taxonomy_version: Option<String>,
    pub display_name: Option<String>,
    #[allow(dead_code)]
    pub is_noisy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SiteDictionaryOverrideTargetKind {
    ExactDomain,
    Host,
    UrlPrefix,
}

impl SiteDictionaryOverrideTargetKind {
    #[cfg_attr(not(test), allow(dead_code))]
    fn as_str(&self) -> &'static str {
        match self {
            Self::ExactDomain => "exact-domain",
            Self::Host => "host",
            Self::UrlPrefix => "url-prefix",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "exact-domain" => Some(Self::ExactDomain),
            "host" => Some(Self::Host),
            "url-prefix" => Some(Self::UrlPrefix),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SiteDictionaryOverride {
    pub target_kind: SiteDictionaryOverrideTargetKind,
    pub target_value: String,
    pub taxonomy_override: Option<TaxonomyOverride>,
    pub display_name: Option<String>,
    pub search_engine: Option<String>,
    pub is_noisy: bool,
    #[allow(dead_code)]
    pub note: Option<String>,
}

impl SiteDictionaryOverride {
    fn matches(&self, url: &str, registrable_domain: &str, host: &str) -> bool {
        match self.target_kind {
            SiteDictionaryOverrideTargetKind::ExactDomain => {
                registrable_domain == self.target_value
            }
            SiteDictionaryOverrideTargetKind::Host => host == self.target_value,
            SiteDictionaryOverrideTargetKind::UrlPrefix => url.starts_with(&self.target_value),
        }
    }

    fn taxonomy_override(&self) -> Option<TaxonomyOverride> {
        let taxonomy_override = self.taxonomy_override.as_ref()?;
        Some(taxonomy_override.clone())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub(crate) struct SiteDictionaryOverrideUpsert {
    pub target_kind: SiteDictionaryOverrideTargetKind,
    pub target_value: String,
    pub domain_category: Option<String>,
    pub page_category: Option<String>,
    pub interaction_kind: Option<String>,
    pub display_name: Option<String>,
    pub search_engine: Option<String>,
    pub is_noisy: bool,
    pub note: Option<String>,
}

const SITE_DICTIONARY_OVERRIDES_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS site_dictionary_overrides (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  target_kind       TEXT NOT NULL,
  target_value      TEXT NOT NULL,
  domain_category   TEXT,
  page_category     TEXT,
  interaction_kind  TEXT,
  display_name      TEXT,
  search_engine     TEXT,
  is_noisy          INTEGER NOT NULL DEFAULT 0,
  note              TEXT,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  UNIQUE(target_kind, target_value)
);
CREATE INDEX IF NOT EXISTS idx_site_dictionary_overrides_target
  ON site_dictionary_overrides(target_kind, target_value);
"#;

pub(crate) fn ensure_site_dictionary_override_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(SITE_DICTIONARY_OVERRIDES_SCHEMA_SQL)
        .context("ensuring site dictionary override schema")
}

pub(crate) fn load_site_dictionary_overrides(
    connection: &Connection,
) -> Result<Vec<SiteDictionaryOverride>> {
    ensure_site_dictionary_override_schema(connection)?;
    let mut statement = connection.prepare(
        "SELECT target_kind,
                target_value,
                domain_category,
                page_category,
                interaction_kind,
                display_name,
                search_engine,
                is_noisy,
                note
         FROM site_dictionary_overrides
         ORDER BY target_kind ASC, target_value ASC",
    )?;
    statement
        .query_map([], |row| {
            let target_kind = row
                .get::<_, String>(0)
                .ok()
                .and_then(|value| SiteDictionaryOverrideTargetKind::from_str(&value))
                .ok_or_else(|| rusqlite::Error::InvalidQuery)?;
            let target_value = row.get::<_, String>(1)?;
            let domain_category = row.get::<_, Option<String>>(2)?;
            let page_category = row.get::<_, Option<String>>(3)?;
            let interaction_kind = row.get::<_, Option<String>>(4)?;
            let taxonomy_override = build_taxonomy_override(
                &target_kind,
                &target_value,
                domain_category.as_deref(),
                page_category.as_deref(),
                interaction_kind.as_deref(),
                row.get::<_, Option<String>>(8)?.clone(),
            );
            Ok(SiteDictionaryOverride {
                target_kind,
                target_value,
                taxonomy_override,
                display_name: row.get(5)?,
                search_engine: row.get(6)?,
                is_noisy: row.get::<_, i64>(7)? != 0,
                note: row.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn upsert_site_dictionary_override(
    connection: &Connection,
    override_rule: &SiteDictionaryOverrideUpsert,
) -> Result<()> {
    ensure_site_dictionary_override_schema(connection)?;
    let now = now_rfc3339();
    connection.execute(
        "INSERT INTO site_dictionary_overrides (
             target_kind,
             target_value,
             domain_category,
             page_category,
             interaction_kind,
             display_name,
             search_engine,
             is_noisy,
             note,
             created_at,
             updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(target_kind, target_value) DO UPDATE SET
             domain_category = excluded.domain_category,
             page_category = excluded.page_category,
             interaction_kind = excluded.interaction_kind,
             display_name = excluded.display_name,
             search_engine = excluded.search_engine,
             is_noisy = excluded.is_noisy,
             note = excluded.note,
             updated_at = excluded.updated_at",
        params![
            override_rule.target_kind.as_str(),
            override_rule.target_value,
            override_rule.domain_category,
            override_rule.page_category,
            override_rule.interaction_kind,
            override_rule.display_name,
            override_rule.search_engine,
            i64::from(override_rule.is_noisy),
            override_rule.note,
            now,
        ],
    )?;
    Ok(())
}

pub(crate) fn classify_visit(
    url: &str,
    title: Option<&str>,
    query: Option<&str>,
    has_canonical_search_term: bool,
    external_referrer_url: Option<&str>,
    from_visit: Option<i64>,
    overrides: &[SiteDictionaryOverride],
) -> SiteDictionaryEntry {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed.host_str().map(|value| value.trim_end_matches('.').to_ascii_lowercase())
        })
        .unwrap_or_default();
    let registrable_domain_hint = crate::deterministic::registrable_domain_for_host(&host);
    let taxonomy_overrides =
        overrides.iter().filter_map(SiteDictionaryOverride::taxonomy_override).collect::<Vec<_>>();
    let analysis = analyze_visit(
        VisitAnalysisInput {
            url,
            title,
            query,
            has_canonical_search_term,
            external_referrer_url,
            from_visit,
        },
        &taxonomy_overrides,
    );
    let normalized_url = analysis.normalized_url;
    let registrable_domain = normalized_url
        .as_ref()
        .map(|value| value.registrable_domain.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or(registrable_domain_hint);
    let canonical_url = normalized_url
        .as_ref()
        .map(|value| value.canonical_url.clone())
        .unwrap_or_else(|| url.to_string());
    let search_query = query
        .map(normalize_query)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_url.as_ref().and_then(|value| value.search_query.clone()));
    let search_engine = search_query
        .as_ref()
        .and(normalized_url.as_ref())
        .map(|value| search_engine_id(&value.registrable_domain, &value.host));
    let matched_override = overrides
        .iter()
        .find(|override_rule| override_rule.matches(url, &registrable_domain, &host));

    SiteDictionaryEntry {
        canonical_url,
        registrable_domain: registrable_domain.clone(),
        domain_category: domain_category_key(analysis.taxonomy.domain_category).to_string(),
        page_category: page_category_key(analysis.taxonomy.page_category).to_string(),
        search_engine: matched_override
            .and_then(|override_rule| override_rule.search_engine.clone())
            .or(search_engine),
        search_query,
        evidence_tier: analysis.evidence.tier.as_str().to_string(),
        taxonomy_source: analysis.taxonomy.source.as_str().to_string(),
        taxonomy_pack: analysis.taxonomy.rule_pack,
        taxonomy_version: Some(analysis.taxonomy.version),
        display_name: matched_override
            .and_then(|override_rule| override_rule.display_name.clone())
            .or_else(|| display_name_for_domain(&registrable_domain)),
        is_noisy: matched_override.is_some_and(|override_rule| override_rule.is_noisy),
    }
}

pub(crate) fn normalize_query(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_lowercase()
}

pub(crate) fn display_name_for_domain(domain: &str) -> Option<String> {
    match domain {
        "bilibili.com" => Some("BiliBili".to_string()),
        "developer.mozilla.org" | "mozilla.org" => Some("MDN".to_string()),
        "github.com" => Some("GitHub".to_string()),
        "google.com" => Some("Google".to_string()),
        "stackoverflow.com" => Some("Stack Overflow".to_string()),
        "sqlite.org" => Some("SQLite".to_string()),
        "youtube.com" => Some("YouTube".to_string()),
        _ => None,
    }
}

pub(crate) fn display_name_for_search_engine(engine: &str) -> Option<String> {
    match engine {
        "baidu" => Some("Baidu".to_string()),
        "bing" => Some("Bing".to_string()),
        "brave" => Some("Brave Search".to_string()),
        "duckduckgo" => Some("DuckDuckGo".to_string()),
        "google" => Some("Google".to_string()),
        "sogou" => Some("Sogou".to_string()),
        "so" => Some("360 Search".to_string()),
        "yahoo" => Some("Yahoo".to_string()),
        "yandex" => Some("Yandex".to_string()),
        _ => None,
    }
}

pub(crate) fn search_engine_id(registrable_domain: &str, host: &str) -> String {
    match registrable_domain {
        "baidu.com" => "baidu",
        "bing.com" => "bing",
        "brave.com" if host == "search.brave.com" => "brave",
        "duckduckgo.com" => "duckduckgo",
        "google.com" => "google",
        "sogou.com" => "sogou",
        "so.com" => "so",
        "yahoo.com" => "yahoo",
        "yandex.ru" => "yandex",
        _ => host,
    }
    .to_string()
}

fn domain_category_key(category: DomainCategory) -> &'static str {
    match category {
        DomainCategory::Ai => "ai",
        DomainCategory::Community => "community",
        DomainCategory::Developer => "developer",
        DomainCategory::Docs => "docs",
        DomainCategory::Education => "education",
        DomainCategory::Entertainment => "entertainment",
        DomainCategory::Finance => "finance",
        DomainCategory::News => "news",
        DomainCategory::Search => "search",
        DomainCategory::Shopping => "shopping",
        DomainCategory::Social => "social",
        DomainCategory::Travel => "travel",
        DomainCategory::Video => "video",
        DomainCategory::Work => "work",
        DomainCategory::Unknown => "unknown",
    }
}

fn page_category_key(category: PageCategory) -> &'static str {
    match category {
        PageCategory::SearchResults => "search_results",
        PageCategory::DocsPage => "docs_page",
        PageCategory::Repo => "repo",
        PageCategory::Issue => "issue",
        PageCategory::PullRequest => "pull_request",
        PageCategory::ForumThread => "forum_thread",
        PageCategory::ProductPage => "product_page",
        PageCategory::CategoryPage => "category_page",
        PageCategory::VideoPage => "video_page",
        PageCategory::ArticlePage => "article_page",
        PageCategory::Profile => "profile",
        PageCategory::Dashboard => "dashboard",
        PageCategory::Home => "home",
        PageCategory::Unknown => "unknown",
    }
}

fn build_taxonomy_override(
    target_kind: &SiteDictionaryOverrideTargetKind,
    target_value: &str,
    domain_category: Option<&str>,
    page_category: Option<&str>,
    interaction_kind: Option<&str>,
    note: Option<String>,
) -> Option<TaxonomyOverride> {
    Some(TaxonomyOverride {
        target: match target_kind {
            SiteDictionaryOverrideTargetKind::ExactDomain => TaxonomyOverrideTarget::ExactDomain,
            SiteDictionaryOverrideTargetKind::Host => TaxonomyOverrideTarget::Host,
            SiteDictionaryOverrideTargetKind::UrlPrefix => TaxonomyOverrideTarget::UrlPrefix,
        },
        value: target_value.to_string(),
        domain_category: parse_domain_category(domain_category?)?,
        page_category: parse_page_category(page_category?)?,
        interaction_kind: parse_interaction_kind(interaction_kind?)?,
        note,
    })
}

fn parse_domain_category(value: &str) -> Option<DomainCategory> {
    Some(match value {
        "ai" => DomainCategory::Ai,
        "community" => DomainCategory::Community,
        "developer" => DomainCategory::Developer,
        "docs" => DomainCategory::Docs,
        "education" => DomainCategory::Education,
        "entertainment" => DomainCategory::Entertainment,
        "finance" => DomainCategory::Finance,
        "news" => DomainCategory::News,
        "search" => DomainCategory::Search,
        "shopping" => DomainCategory::Shopping,
        "social" => DomainCategory::Social,
        "travel" => DomainCategory::Travel,
        "video" => DomainCategory::Video,
        "work" => DomainCategory::Work,
        "unknown" => DomainCategory::Unknown,
        _ => return None,
    })
}

fn parse_page_category(value: &str) -> Option<PageCategory> {
    Some(match value {
        "search_results" | "search-results" => PageCategory::SearchResults,
        "docs_page" | "docs-page" => PageCategory::DocsPage,
        "repo" => PageCategory::Repo,
        "issue" => PageCategory::Issue,
        "pull_request" | "pull-request" => PageCategory::PullRequest,
        "forum_thread" | "forum-thread" => PageCategory::ForumThread,
        "product_page" | "product-page" => PageCategory::ProductPage,
        "category_page" | "category-page" => PageCategory::CategoryPage,
        "video_page" | "video-page" => PageCategory::VideoPage,
        "article_page" | "article-page" => PageCategory::ArticlePage,
        "profile" => PageCategory::Profile,
        "dashboard" => PageCategory::Dashboard,
        "home" => PageCategory::Home,
        "unknown" => PageCategory::Unknown,
        _ => return None,
    })
}

fn parse_interaction_kind(value: &str) -> Option<InteractionKind> {
    Some(match value {
        "compare" => InteractionKind::Compare,
        "discover" => InteractionKind::Discover,
        "discuss" => InteractionKind::Discuss,
        "learn" => InteractionKind::Learn,
        "manage" => InteractionKind::Manage,
        "resolve" => InteractionKind::Resolve,
        "transact" => InteractionKind::Transact,
        "watch" => InteractionKind::Watch,
        "unknown" => InteractionKind::Unknown,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        SiteDictionaryOverrideTargetKind, SiteDictionaryOverrideUpsert, classify_visit,
        ensure_site_dictionary_override_schema, load_site_dictionary_overrides, normalize_query,
        upsert_site_dictionary_override,
    };
    use rusqlite::Connection;

    #[test]
    fn classify_visit_extracts_search_engine_and_query() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_site_dictionary_override_schema(&connection).expect("override schema");
        let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
        let entry = classify_visit(
            "https://www.google.com/search?q=sqlite+wal+checkpoint&utm_source=test",
            Some("sqlite wal checkpoint - Google Search"),
            None,
            true,
            None,
            None,
            &overrides,
        );
        assert_eq!(entry.registrable_domain, "google.com");
        assert_eq!(entry.search_engine.as_deref(), Some("google"));
        assert_eq!(entry.search_query.as_deref(), Some("sqlite wal checkpoint"));
        assert!(entry.canonical_url.contains("q=sqlite+wal+checkpoint"));
        assert!(!entry.canonical_url.contains("utm_source"));
    }

    #[test]
    fn normalize_query_canonicalizes_whitespace() {
        assert_eq!(normalize_query("  Tauri   v2   Docs "), "tauri v2 docs");
    }

    #[test]
    fn persisted_user_override_beats_builtin_rule_pack() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        upsert_site_dictionary_override(
            &connection,
            &SiteDictionaryOverrideUpsert {
                target_kind: SiteDictionaryOverrideTargetKind::ExactDomain,
                target_value: "github.com".to_string(),
                domain_category: Some("work".to_string()),
                page_category: Some("dashboard".to_string()),
                interaction_kind: Some("manage".to_string()),
                display_name: Some("GitHub Work".to_string()),
                search_engine: None,
                is_noisy: false,
                note: Some("tests".to_string()),
            },
        )
        .expect("insert override");
        let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");

        let entry = classify_visit(
            "https://github.com/example/repo/issues/42",
            Some("Issue 42"),
            None,
            false,
            None,
            Some(1),
            &overrides,
        );

        assert_eq!(entry.taxonomy_source, "user-override");
        assert_eq!(entry.domain_category, "work");
        assert_eq!(entry.page_category, "dashboard");
        assert_eq!(entry.display_name.as_deref(), Some("GitHub Work"));
    }
}
