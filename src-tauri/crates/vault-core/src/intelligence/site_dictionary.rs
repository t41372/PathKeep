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
    models::{SearchEngineRule, SearchEngineRuleInput},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use std::collections::HashMap;

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

const SEARCH_ENGINE_RULES_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS search_engine_rules (
  rule_id           TEXT PRIMARY KEY,
  engine_id         TEXT NOT NULL,
  display_name      TEXT NOT NULL,
  host_pattern      TEXT NOT NULL,
  path_prefix       TEXT,
  query_param_key   TEXT NOT NULL,
  enabled           INTEGER NOT NULL DEFAULT 1,
  note              TEXT,
  example_url       TEXT,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_search_engine_rules_host
  ON search_engine_rules(host_pattern, enabled);
"#;

#[derive(Debug, Clone)]
pub(crate) struct SearchEngineRuleConfig {
    rule_id: String,
    engine_id: String,
    display_name: String,
    host_pattern: String,
    path_prefix: Option<String>,
    query_param_key: String,
    enabled: bool,
    note: Option<String>,
    example_url: Option<String>,
    built_in: bool,
}

#[derive(Debug, Clone, Copy)]
struct BuiltinSearchEngineRule {
    rule_id: &'static str,
    engine_id: &'static str,
    display_name: &'static str,
    host_pattern: &'static str,
    path_prefix: Option<&'static str>,
    query_param_key: &'static str,
    example_url: &'static str,
}

#[derive(Debug, Clone)]
struct SearchQueryMatch {
    engine_id: String,
    display_name: String,
    query: String,
}

const BUILTIN_SEARCH_ENGINE_RULES: &[BuiltinSearchEngineRule] = &[
    BuiltinSearchEngineRule {
        rule_id: "builtin:google",
        engine_id: "google",
        display_name: "Google",
        host_pattern: "google.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://www.google.com/search?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:bing",
        engine_id: "bing",
        display_name: "Bing",
        host_pattern: "bing.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://www.bing.com/search?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:duckduckgo",
        engine_id: "duckduckgo",
        display_name: "DuckDuckGo",
        host_pattern: "duckduckgo.com",
        path_prefix: None,
        query_param_key: "q",
        example_url: "https://duckduckgo.com/?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:brave",
        engine_id: "brave",
        display_name: "Brave Search",
        host_pattern: "search.brave.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://search.brave.com/search?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:baidu",
        engine_id: "baidu",
        display_name: "Baidu",
        host_pattern: "baidu.com",
        path_prefix: Some("/s"),
        query_param_key: "wd",
        example_url: "https://www.baidu.com/s?wd=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:sogou",
        engine_id: "sogou",
        display_name: "Sogou",
        host_pattern: "sogou.com",
        path_prefix: Some("/web"),
        query_param_key: "query",
        example_url: "https://www.sogou.com/web?query=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:so",
        engine_id: "so",
        display_name: "360 Search",
        host_pattern: "so.com",
        path_prefix: Some("/s"),
        query_param_key: "q",
        example_url: "https://www.so.com/s?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:yahoo",
        engine_id: "yahoo",
        display_name: "Yahoo",
        host_pattern: "yahoo.com",
        path_prefix: Some("/search"),
        query_param_key: "p",
        example_url: "https://search.yahoo.com/search?p=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:yandex",
        engine_id: "yandex",
        display_name: "Yandex",
        host_pattern: "yandex.ru",
        path_prefix: Some("/search"),
        query_param_key: "text",
        example_url: "https://yandex.ru/search/?text=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:youtube",
        engine_id: "youtube",
        display_name: "YouTube",
        host_pattern: "youtube.com",
        path_prefix: Some("/results"),
        query_param_key: "search_query",
        example_url: "https://www.youtube.com/results?search_query=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:bilibili",
        engine_id: "bilibili",
        display_name: "BiliBili",
        host_pattern: "search.bilibili.com",
        path_prefix: Some("/all"),
        query_param_key: "keyword",
        example_url: "https://search.bilibili.com/all?keyword=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:github",
        engine_id: "github",
        display_name: "GitHub",
        host_pattern: "github.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://github.com/search?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:reddit",
        engine_id: "reddit",
        display_name: "Reddit",
        host_pattern: "reddit.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://www.reddit.com/search/?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:zhihu",
        engine_id: "zhihu",
        display_name: "Zhihu",
        host_pattern: "zhihu.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://www.zhihu.com/search?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:taobao",
        engine_id: "taobao",
        display_name: "Taobao",
        host_pattern: "taobao.com",
        path_prefix: Some("/search"),
        query_param_key: "q",
        example_url: "https://s.taobao.com/search?q=sqlite+wal",
    },
    BuiltinSearchEngineRule {
        rule_id: "builtin:amazon",
        engine_id: "amazon",
        display_name: "Amazon",
        host_pattern: "amazon.com",
        path_prefix: Some("/s"),
        query_param_key: "k",
        example_url: "https://www.amazon.com/s?k=sqlite+wal",
    },
];

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

pub(crate) fn ensure_search_engine_rule_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(SEARCH_ENGINE_RULES_SCHEMA_SQL)
        .context("ensuring search engine rule schema")
}

pub(crate) fn list_search_engine_rules(connection: &Connection) -> Result<Vec<SearchEngineRule>> {
    let mut rules = builtin_search_engine_rules();
    rules.extend(load_custom_search_engine_rules(connection)?);
    rules.sort_by(|left, right| {
        left.built_in
            .cmp(&right.built_in)
            .reverse()
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.rule_id.cmp(&right.rule_id))
    });
    Ok(rules.into_iter().map(SearchEngineRule::from).collect())
}

pub(crate) fn upsert_search_engine_rule(
    connection: &Connection,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    ensure_search_engine_rule_schema(connection)?;
    let now = now_rfc3339();
    let rule_id = input
        .rule_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(generate_custom_search_rule_id);
    connection.execute(
        "INSERT INTO search_engine_rules (
             rule_id,
             engine_id,
             display_name,
             host_pattern,
             path_prefix,
             query_param_key,
             enabled,
             note,
             example_url,
             created_at,
             updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(rule_id) DO UPDATE SET
             engine_id = excluded.engine_id,
             display_name = excluded.display_name,
             host_pattern = excluded.host_pattern,
             path_prefix = excluded.path_prefix,
             query_param_key = excluded.query_param_key,
             enabled = excluded.enabled,
             note = excluded.note,
             example_url = excluded.example_url,
             updated_at = excluded.updated_at",
        params![
            rule_id,
            normalize_engine_id(&input.engine_id),
            input.display_name.trim(),
            normalize_host_pattern(&input.host_pattern),
            normalize_optional_text(input.path_prefix.as_deref()),
            normalize_query_param_key(&input.query_param_key),
            i64::from(input.enabled),
            normalize_optional_text(input.note.as_deref()),
            normalize_optional_text(input.example_url.as_deref()),
            now,
        ],
    )?;
    list_search_engine_rules(connection)
}

pub(crate) fn delete_search_engine_rule(
    connection: &Connection,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    ensure_search_engine_rule_schema(connection)?;
    connection.execute("DELETE FROM search_engine_rules WHERE rule_id = ?1", [rule_id])?;
    list_search_engine_rules(connection)
}

pub(crate) fn load_search_engine_display_names(
    connection: &Connection,
) -> Result<HashMap<String, String>> {
    let rules = list_search_engine_rules(connection)?;
    Ok(rules.into_iter().map(|rule| (rule.engine_id, rule.display_name)).collect::<HashMap<_, _>>())
}

pub(crate) fn display_name_for_search_engine_with_map(
    engine: &str,
    display_names: &HashMap<String, String>,
) -> Option<String> {
    display_names.get(engine).cloned().or_else(|| display_name_for_search_engine(engine))
}

pub(crate) fn load_enabled_search_engine_rules(
    connection: &Connection,
) -> Result<Vec<SearchEngineRuleConfig>> {
    let mut rules = load_custom_search_engine_rules(connection)?
        .into_iter()
        .filter(|rule| rule.enabled)
        .collect::<Vec<_>>();
    rules.extend(builtin_search_engine_rules().into_iter().filter(|rule| rule.enabled));
    Ok(rules)
}

fn load_custom_search_engine_rules(connection: &Connection) -> Result<Vec<SearchEngineRuleConfig>> {
    ensure_search_engine_rule_schema(connection)?;
    let mut statement = connection.prepare(
        "SELECT rule_id,
                engine_id,
                display_name,
                host_pattern,
                path_prefix,
                query_param_key,
                enabled,
                note,
                example_url
         FROM search_engine_rules
         ORDER BY updated_at DESC, rule_id DESC",
    )?;
    statement
        .query_map([], |row| {
            Ok(SearchEngineRuleConfig {
                rule_id: row.get(0)?,
                engine_id: row.get(1)?,
                display_name: row.get(2)?,
                host_pattern: row.get(3)?,
                path_prefix: row.get(4)?,
                query_param_key: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                note: row.get(7)?,
                example_url: row.get(8)?,
                built_in: false,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn builtin_search_engine_rules() -> Vec<SearchEngineRuleConfig> {
    BUILTIN_SEARCH_ENGINE_RULES
        .iter()
        .map(|rule| SearchEngineRuleConfig {
            rule_id: rule.rule_id.to_string(),
            engine_id: rule.engine_id.to_string(),
            display_name: rule.display_name.to_string(),
            host_pattern: rule.host_pattern.to_string(),
            path_prefix: rule.path_prefix.map(str::to_string),
            query_param_key: rule.query_param_key.to_string(),
            enabled: true,
            note: None,
            example_url: Some(rule.example_url.to_string()),
            built_in: true,
        })
        .collect()
}

pub(crate) fn classify_visit(
    url: &str,
    title: Option<&str>,
    query: Option<&str>,
    has_canonical_search_term: bool,
    external_referrer_url: Option<&str>,
    from_visit: Option<i64>,
    overrides: &[SiteDictionaryOverride],
    search_rules: &[SearchEngineRuleConfig],
) -> SiteDictionaryEntry {
    let parsed_url = reqwest::Url::parse(url).ok();
    let host = parsed_url
        .as_ref()
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
    let matched_search_rule = parsed_url
        .as_ref()
        .and_then(|parsed| match_search_engine_rule(parsed, &registrable_domain, search_rules));
    let generic_search_query = normalized_url.as_ref().and_then(|value| value.search_query.clone());
    let search_query = query
        .map(normalize_query)
        .filter(|value| !value.is_empty())
        .or_else(|| matched_search_rule.as_ref().map(|rule| normalize_query(&rule.query)))
        .or(generic_search_query);
    let generic_search_engine = search_query
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
            .or_else(|| matched_search_rule.as_ref().map(|rule| rule.engine_id.clone()))
            .or(generic_search_engine),
        search_query,
        evidence_tier: analysis.evidence.tier.as_str().to_string(),
        taxonomy_source: analysis.taxonomy.source.as_str().to_string(),
        taxonomy_pack: analysis.taxonomy.rule_pack,
        taxonomy_version: Some(analysis.taxonomy.version),
        display_name: matched_override
            .and_then(|override_rule| override_rule.display_name.clone())
            .or_else(|| matched_search_rule.as_ref().map(|rule| rule.display_name.clone()))
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
        "amazon" => Some("Amazon".to_string()),
        "baidu" => Some("Baidu".to_string()),
        "bilibili" => Some("BiliBili".to_string()),
        "bing" => Some("Bing".to_string()),
        "brave" => Some("Brave Search".to_string()),
        "duckduckgo" => Some("DuckDuckGo".to_string()),
        "github" => Some("GitHub".to_string()),
        "google" => Some("Google".to_string()),
        "reddit" => Some("Reddit".to_string()),
        "sogou" => Some("Sogou".to_string()),
        "so" => Some("360 Search".to_string()),
        "taobao" => Some("Taobao".to_string()),
        "youtube" => Some("YouTube".to_string()),
        "yahoo" => Some("Yahoo".to_string()),
        "yandex" => Some("Yandex".to_string()),
        "zhihu" => Some("Zhihu".to_string()),
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

fn match_search_engine_rule(
    parsed_url: &reqwest::Url,
    registrable_domain: &str,
    search_rules: &[SearchEngineRuleConfig],
) -> Option<SearchQueryMatch> {
    let host = parsed_url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let path = parsed_url.path();
    for rule in search_rules.iter().filter(|rule| rule.enabled) {
        if !host_matches_rule(&host, registrable_domain, &rule.host_pattern)
            || !path_matches_rule(path, rule.path_prefix.as_deref())
        {
            continue;
        }
        let value = parsed_url
            .query_pairs()
            .find(|(key, _)| key.eq_ignore_ascii_case(rule.query_param_key.as_str()))
            .map(|(_, value)| normalize_query_spacing(&value.replace('+', " ")))
            .filter(|value| !value.is_empty())?;
        return Some(SearchQueryMatch {
            engine_id: rule.engine_id.clone(),
            display_name: rule.display_name.clone(),
            query: value,
        });
    }
    None
}

fn host_matches_rule(host: &str, registrable_domain: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    if pattern.is_empty() {
        return false;
    }
    host == pattern
        || host.ends_with(&format!(".{pattern}"))
        || registrable_domain == pattern
        || registrable_domain.ends_with(&format!(".{pattern}"))
}

fn path_matches_rule(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|value| path.starts_with(value))
}

fn normalize_query_spacing(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

fn normalize_host_pattern(raw: &str) -> String {
    raw.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn normalize_query_param_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn normalize_optional_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim).filter(|value| !value.is_empty()).map(ToString::to_string)
}

fn normalize_engine_id(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace(' ', "-")
}

fn generate_custom_search_rule_id() -> String {
    format!("custom-search-rule-{}", Utc::now().timestamp_millis())
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

impl From<SearchEngineRuleConfig> for SearchEngineRule {
    fn from(rule: SearchEngineRuleConfig) -> Self {
        Self {
            rule_id: rule.rule_id,
            engine_id: rule.engine_id,
            display_name: rule.display_name,
            host_pattern: rule.host_pattern,
            path_prefix: rule.path_prefix,
            query_param_key: rule.query_param_key,
            enabled: rule.enabled,
            note: rule.note,
            example_url: rule.example_url,
            built_in: rule.built_in,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SiteDictionaryOverrideTargetKind, SiteDictionaryOverrideUpsert, classify_visit,
        ensure_search_engine_rule_schema, ensure_site_dictionary_override_schema,
        list_search_engine_rules, load_enabled_search_engine_rules, load_site_dictionary_overrides,
        normalize_query, upsert_search_engine_rule, upsert_site_dictionary_override,
    };
    use crate::models::SearchEngineRuleInput;
    use rusqlite::Connection;

    #[test]
    fn classify_visit_extracts_search_engine_and_query() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_site_dictionary_override_schema(&connection).expect("override schema");
        ensure_search_engine_rule_schema(&connection).expect("search engine schema");
        let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
        let rules = load_enabled_search_engine_rules(&connection).expect("load rules");
        let entry = classify_visit(
            "https://www.google.com/search?q=sqlite+wal+checkpoint&utm_source=test",
            Some("sqlite wal checkpoint - Google Search"),
            None,
            true,
            None,
            None,
            &overrides,
            &rules,
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
        let rules = load_enabled_search_engine_rules(&connection).expect("load rules");

        let entry = classify_visit(
            "https://github.com/example/repo/issues/42",
            Some("Issue 42"),
            None,
            false,
            None,
            Some(1),
            &overrides,
            &rules,
        );

        assert_eq!(entry.taxonomy_source, "user-override");
        assert_eq!(entry.domain_category, "work");
        assert_eq!(entry.page_category, "dashboard");
        assert_eq!(entry.display_name.as_deref(), Some("GitHub Work"));
    }

    #[test]
    fn builtin_rules_cover_bilibili_and_github_search() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_site_dictionary_override_schema(&connection).expect("override schema");
        ensure_search_engine_rule_schema(&connection).expect("search engine schema");
        let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
        let rules = load_enabled_search_engine_rules(&connection).expect("load rules");

        let bilibili = classify_visit(
            "https://search.bilibili.com/all?keyword=sqlite+wal",
            Some("sqlite wal - 哔哩哔哩"),
            None,
            false,
            None,
            None,
            &overrides,
            &rules,
        );
        assert_eq!(bilibili.search_engine.as_deref(), Some("bilibili"));
        assert_eq!(bilibili.search_query.as_deref(), Some("sqlite wal"));

        let github = classify_visit(
            "https://github.com/search?q=pathkeep+sqlite",
            Some("Repository search results"),
            None,
            false,
            None,
            None,
            &overrides,
            &rules,
        );
        assert_eq!(github.search_engine.as_deref(), Some("github"));
        assert_eq!(github.search_query.as_deref(), Some("pathkeep sqlite"));
    }

    #[test]
    fn custom_search_rules_override_builtin_matching() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_site_dictionary_override_schema(&connection).expect("override schema");
        upsert_search_engine_rule(
            &connection,
            &SearchEngineRuleInput {
                rule_id: Some("custom-github-code".to_string()),
                engine_id: "github-code".to_string(),
                display_name: "GitHub Code".to_string(),
                host_pattern: "github.com".to_string(),
                path_prefix: Some("/search".to_string()),
                query_param_key: "q".to_string(),
                enabled: true,
                note: Some("custom".to_string()),
                example_url: Some("https://github.com/search?q=pathkeep".to_string()),
            },
        )
        .expect("upsert custom rule");
        let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
        let rules = load_enabled_search_engine_rules(&connection).expect("load rules");

        let entry = classify_visit(
            "https://github.com/search?q=pathkeep+sqlite",
            Some("Repository search results"),
            None,
            false,
            None,
            None,
            &overrides,
            &rules,
        );
        assert_eq!(entry.search_engine.as_deref(), Some("github-code"));
        assert_eq!(entry.search_query.as_deref(), Some("pathkeep sqlite"));
    }

    #[test]
    fn list_rules_merges_builtin_and_custom_rules() {
        let connection = Connection::open_in_memory().expect("in memory sqlite");
        upsert_search_engine_rule(
            &connection,
            &SearchEngineRuleInput {
                rule_id: Some("custom-amazon-de".to_string()),
                engine_id: "amazon-de".to_string(),
                display_name: "Amazon DE".to_string(),
                host_pattern: "amazon.de".to_string(),
                path_prefix: Some("/s".to_string()),
                query_param_key: "k".to_string(),
                enabled: true,
                note: None,
                example_url: None,
            },
        )
        .expect("upsert custom rule");

        let rules = list_search_engine_rules(&connection).expect("list rules");
        assert!(rules.iter().any(|rule| rule.rule_id == "builtin:bilibili"));
        assert!(rules.iter().any(|rule| rule.rule_id == "custom-amazon-de"));
    }
}
