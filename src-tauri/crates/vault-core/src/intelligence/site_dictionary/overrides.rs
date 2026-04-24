//! SQLite ownership for user-managed site-dictionary overrides.
//!
//! ## Responsibilities
//! - Bootstrap and read the `site_dictionary_overrides` table.
//! - Persist user override edits with stable conflict behavior.
//! - Translate persisted category keys into `visit_taxonomy` override enums.
//!
//! ## Not responsible for
//! - Applying overrides to visits; classification owns the matching step.
//! - Owning built-in taxonomy rules or search-engine rule persistence.
//! - Surfacing Settings UI payloads.
//!
//! ## Dependencies
//! - `rusqlite` for the derived intelligence SQLite plane.
//! - `utils::now_rfc3339` for stable audit timestamps.
//! - `visit_taxonomy` enum contracts.
//!
//! ## Performance notes
//! - Overrides are loaded once per classifier batch and then matched in memory.
//!   The table stays indexed by target kind/value for future selective reads.

use super::types::{
    SiteDictionaryOverride, SiteDictionaryOverrideTargetKind, SiteDictionaryOverrideUpsert,
};
use crate::{
    utils::now_rfc3339,
    visit_taxonomy::{
        DomainCategory, InteractionKind, PageCategory, TaxonomyOverride, TaxonomyOverrideTarget,
    },
};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};

/// Schema for persisted user override rules. The unique target pair ensures one
/// edit replaces the previous rule instead of creating ambiguous matches.
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

/// Ensures the override table exists before classification or Settings reads.
/// This is idempotent and safe to call from each public owner boundary.
pub(crate) fn ensure_site_dictionary_override_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(SITE_DICTIONARY_OVERRIDES_SCHEMA_SQL)
        .context("ensuring site dictionary override schema")
}

/// Loads user overrides in deterministic order so equal-match behavior remains
/// stable across rebuilds and test runs.
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

/// Inserts or replaces one user override without changing the target identity.
/// Empty category strings are not normalized here because callers are expected
/// to send explicit taxonomy keys or `None`.
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

/// Converts optional persisted category keys into one all-or-nothing taxonomy
/// override. Partial taxonomy rows are ignored to avoid creating mixed
/// user/built-in evidence with unclear semantics.
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

/// Parses a persisted domain category key into the taxonomy enum used by
/// `visit_taxonomy`.
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

/// Parses persisted page category keys while accepting both underscore and
/// hyphen spellings used by earlier operator-facing drafts.
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

/// Parses the persisted interaction intent key for an override row.
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
