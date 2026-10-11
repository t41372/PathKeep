//! Failure modes: edits vanish, disabled rules classify visits, or reads bootstrap storage.
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
//! - The Insights database, through `vault_store::Session`.
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
use anyhow::Result;
use vault_store::{Session, params_owned as params};

/// Schema for persisted user override rules. The unique target pair ensures one
/// edit replaces the previous rule instead of creating ambiguous matches.
/// Ensures the override table exists before classification or Settings reads.
/// This is idempotent and safe to call from each public owner boundary.
/// Loads user overrides in deterministic order so equal-match behavior remains
/// stable across rebuilds and test runs.
pub(crate) fn load_site_dictionary_overrides(
    connection: &Session<'_>,
) -> Result<Vec<SiteDictionaryOverride>> {
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
        .query_map((), |row| {
            let target_kind = row
                .get::<String>(0)
                .ok()
                .and_then(|value| SiteDictionaryOverrideTargetKind::from_str(&value))
                .ok_or_else(|| {
                    vault_store::Error::Operation(Box::new(std::io::Error::other(
                        "unknown site dictionary target",
                    )))
                })?;
            let target_value = row.get::<String>(1)?;
            let domain_category = row.get::<Option<String>>(2)?;
            let page_category = row.get::<Option<String>>(3)?;
            let interaction_kind = row.get::<Option<String>>(4)?;
            let taxonomy_override = build_taxonomy_override(
                &target_kind,
                &target_value,
                domain_category.as_deref(),
                page_category.as_deref(),
                interaction_kind.as_deref(),
                row.get::<Option<String>>(8)?.clone(),
            );
            Ok(SiteDictionaryOverride {
                target_kind,
                target_value,
                taxonomy_override,
                display_name: row.get(5)?,
                search_engine: row.get(6)?,
                is_noisy: row.get::<i64>(7)? != 0,
                note: row.get(8)?,
            })
        })?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Inserts or replaces one user override without changing the target identity.
/// Empty category strings are not normalized here because callers are expected
/// to send explicit taxonomy keys or `None`.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn upsert_site_dictionary_override(
    connection: &Session<'_>,
    override_rule: &SiteDictionaryOverrideUpsert,
) -> Result<()> {
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

/// Converts persisted category keys into a taxonomy override. A row needs a site category; its
/// page kind and interaction are optional, and the rules give those it leaves out (a site category
/// set on Insights' site page sets only that). A key PathKeep does not know drops the row.
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
        page_category: match page_category {
            Some(value) => Some(parse_page_category(value)?),
            None => None,
        },
        interaction_kind: match interaction_kind {
            Some(value) => Some(parse_interaction_kind(value)?),
            None => None,
        },
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
