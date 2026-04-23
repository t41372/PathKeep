//! Navigation-path and hub-page Core Intelligence reads.
//!
//! ## Responsibilities
//! - Reconstruct the parent chain for one visit so the UI can explain how the
//!   user reached a page.
//! - Surface hub pages by counting how often pages anchor distinct search
//!   trails in a bounded window.
//! - Keep archive visit lookup helpers out of the parent intelligence module.
//!
//! ## Not responsible for
//! - Session/trail detail reads.
//! - Domain deep-dive or query-family surfaces.
//! - Rebuilding any derived intelligence table.
//!
//! ## Dependencies
//! - Parent-module date-range helpers, archive visit-row decoder, and schema
//!   bootstrap.
//! - `search_trail_members`, `visit_derived_facts`, and archive visit/url
//!   joins.
//!
//! ## Performance notes
//! - Navigation-path reconstruction walks a single ancestry chain and stops on
//!   cycles or missing parents.
//! - Hub-page reads aggregate inside SQLite and only return the requested top-N
//!   rows.

use super::{VisitRecord, date_range_bounds, ensure_core_intelligence_schema, visit_from_row};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{AppConfig, HubPage, NavigationPath, NavigationPathStep, TopSitesRequest},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;

/// Reconstructs the visible parent chain for one visit using source-visit ids
/// inside the same source profile.
pub fn get_navigation_path(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    visit_id: i64,
) -> Result<NavigationPath> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut steps = Vec::<NavigationPathStep>::new();
    let mut current = load_navigation_visit(&connection, visit_id)?
        .with_context(|| format!("visit {visit_id} was not found"))?;
    let source_profile_id = current.source_profile_id;
    steps.push(NavigationPathStep {
        visit_id: current.visit_id,
        url: current.url.clone(),
        title: current.title.clone(),
        visit_time_ms: current.visit_time_ms,
        depth: 0,
    });
    let mut depth = 1_i64;
    let mut seen = HashSet::<i64>::from([current.visit_id]);
    while let Some(parent_source_visit_id) = current.from_visit {
        let Some(parent) = load_navigation_visit_by_source(
            &connection,
            source_profile_id,
            parent_source_visit_id,
        )?
        else {
            break;
        };
        if !seen.insert(parent.visit_id) {
            break;
        }
        steps.push(NavigationPathStep {
            visit_id: parent.visit_id,
            url: parent.url.clone(),
            title: parent.title.clone(),
            visit_time_ms: parent.visit_time_ms,
            depth,
        });
        current = parent;
        depth += 1;
    }
    steps.reverse();
    for (index, step) in steps.iter_mut().enumerate() {
        step.depth = index as i64;
    }
    Ok(NavigationPath { target_visit_id: visit_id, steps })
}

/// Returns the most frequently referenced landing pages across search trails in
/// the requested scope.
pub fn get_hub_pages(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<Vec<HubPage>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.canonical_url, MAX(urls.title), visit_derived_facts.registrable_domain,
                COUNT(DISTINCT search_trail_members.trail_id)
         FROM search_trail_members
         JOIN archive.visits AS visits ON visits.id = search_trail_members.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         GROUP BY visit_derived_facts.canonical_url, visit_derived_facts.registrable_domain
         ORDER BY COUNT(DISTINCT search_trail_members.trail_id) DESC, visit_derived_facts.canonical_url ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.limit.unwrap_or(10).max(1) as i64
            ],
            |row| {
                Ok(HubPage {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    registrable_domain: row.get(2)?,
                    trail_reference_count: row.get(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Loads the canonical visit row needed to start reconstructing a navigation
/// path.
fn load_navigation_visit(connection: &Connection, visit_id: i64) -> Result<Option<VisitRecord>> {
    connection
        .query_row(
            "SELECT visits.id, source_profiles.profile_key, visits.source_profile_id, CAST(visits.source_visit_id AS INTEGER),
                    urls.id, urls.url, urls.title, visits.visit_time_ms, visits.from_visit, visits.transition_type,
                    visits.external_referrer_url
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.id = ?1",
            [visit_id],
            visit_from_row,
        )
        .optional()
        .map_err(Into::into)
}

/// Loads the parent visit that shares the same source profile and
/// `source_visit_id`.
fn load_navigation_visit_by_source(
    connection: &Connection,
    source_profile_id: i64,
    source_visit_id: i64,
) -> Result<Option<VisitRecord>> {
    connection
        .query_row(
            "SELECT visits.id, source_profiles.profile_key, visits.source_profile_id, CAST(visits.source_visit_id AS INTEGER),
                    urls.id, urls.url, urls.title, visits.visit_time_ms, visits.from_visit, visits.transition_type,
                    visits.external_referrer_url
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.source_profile_id = ?1
               AND CAST(visits.source_visit_id AS INTEGER) = ?2
             LIMIT 1",
            params![source_profile_id, source_visit_id],
            visit_from_row,
        )
        .optional()
        .map_err(Into::into)
}
