//! Search-engine rule persistence and merged built-in rule ownership.
//!
//! ## Responsibilities
//! - Bootstrap and maintain `search_engine_rules` for user-defined rules.
//! - Merge built-in and custom search rules for rebuild classification.
//! - Return Settings-facing `SearchEngineRule` payloads without leaking
//!   classifier internals.
//!
//! ## Not responsible for
//! - Matching a visit URL against a loaded rule list.
//! - Defining generic URL normalization or taxonomy categories.
//! - Owning the frontend form validation grammar for Settings.
//!
//! ## Dependencies
//! - `models::{SearchEngineRule, SearchEngineRuleInput}` for public payloads.
//! - `rusqlite` for custom rule storage.
//! - `chrono` only for generating new custom rule ids.
//!
//! ## Performance notes
//! - Custom rules are tiny relative to visit volume and are loaded once per
//!   rebuild batch; classification never performs per-visit SQLite reads.
//! - Built-in rules are synthesized in memory so default search coverage does
//!   not require seed migrations.

use super::{classification::display_name_for_search_engine, types::SearchEngineRuleConfig};
use crate::{
    models::{SearchEngineRule, SearchEngineRuleInput},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use std::collections::HashMap;

/// Schema for user-defined search rules. Built-ins are intentionally not
/// persisted so app upgrades can ship corrected defaults without migrations.
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

/// Compact checked-in definition for one built-in search engine rule.
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

/// Built-in search engine coverage used by both classification and Settings
/// review. The order is not user-facing; list functions sort before returning.
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

/// Ensures the custom-rule table exists before Settings or rebuild consumers
/// request the merged rule set.
pub(crate) fn ensure_search_engine_rule_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(SEARCH_ENGINE_RULES_SCHEMA_SQL)
        .context("ensuring search engine rule schema")
}

/// Returns the Settings-facing merged rule list with built-ins and custom
/// entries sorted predictably for review.
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

/// Inserts or replaces one custom search rule, normalizing host and query-param
/// keys before returning the refreshed merged list.
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

/// Deletes one custom rule and returns the refreshed Settings list. Built-ins
/// are not persisted, so deleting a built-in id is a harmless no-op.
pub(crate) fn delete_search_engine_rule(
    connection: &Connection,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    ensure_search_engine_rule_schema(connection)?;
    connection.execute("DELETE FROM search_engine_rules WHERE rule_id = ?1", [rule_id])?;
    list_search_engine_rules(connection)
}

/// Builds a display-name map from the current merged rule set for aggregate
/// read models that only store engine ids.
pub(crate) fn load_search_engine_display_names(
    connection: &Connection,
) -> Result<HashMap<String, String>> {
    let rules = list_search_engine_rules(connection)?;
    Ok(rules.into_iter().map(|rule| (rule.engine_id, rule.display_name)).collect::<HashMap<_, _>>())
}

/// Resolves a search-engine label from a preloaded map, falling back to
/// hard-coded aliases for legacy rows.
pub(crate) fn display_name_for_search_engine_with_map(
    engine: &str,
    display_names: &HashMap<String, String>,
) -> Option<String> {
    display_names.get(engine).cloned().or_else(|| display_name_for_search_engine(engine))
}

/// Loads only enabled rules for hot-path visit classification, with custom
/// rules first so user edits can override built-in matching.
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

/// Reads custom search rules from SQLite in newest-first order so Settings
/// edits remain easy to inspect before final sorting.
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

/// Synthesizes checked-in search rules into the same shape custom rules use.
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

/// Normalizes a rule host pattern so matching can compare lower-case host keys
/// without allocating alternate forms per visit.
fn normalize_host_pattern(raw: &str) -> String {
    raw.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Normalizes the query-parameter key used to extract search terms.
fn normalize_query_param_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

/// Converts optional user text into a stored value only when it carries real
/// content.
fn normalize_optional_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim).filter(|value| !value.is_empty()).map(ToString::to_string)
}

/// Normalizes the engine id used by rollups and Settings payloads.
fn normalize_engine_id(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace(' ', "-")
}

/// Generates a custom rule id when Settings creates a new rule. Timestamp
/// precision is sufficient because edits happen through a user-facing control,
/// not a high-throughput batch path.
fn generate_custom_search_rule_id() -> String {
    format!("custom-search-rule-{}", Utc::now().timestamp_millis())
}

/// Converts the merged internal rule representation into the public Settings
/// DTO without exposing private classifier-only helpers.
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
