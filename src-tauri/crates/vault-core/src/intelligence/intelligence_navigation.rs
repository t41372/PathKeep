//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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

use super::{VisitRecord, date_range_bounds, visit_from_row};
use crate::models::{HubPage, NavigationPath, NavigationPathStep, TopSitesRequest};
use crate::visit_taxonomy::Opener;
use anyhow::{Context, Result};
use std::collections::HashSet;
use vault_store::{OptionalRow, Session, params_owned as params};

/// Reconstructs the visible parent chain for one visit using source-visit ids
/// inside the same source profile.
pub fn get_navigation_path(vault: &crate::Vault, visit_id: i64) -> Result<NavigationPath> {
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let mut steps = Vec::<NavigationPathStep>::new();
        let mut current = load_navigation_visit(connection, visit_id)?
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
        while let Opener::Visit(parent_source_visit_id) = Opener::of(current.from_visit) {
            let Some(parent) = load_navigation_visit_by_source(
                connection,
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
    })
}

/// Returns the most frequently referenced landing pages across search trails in
/// the requested scope.
pub fn get_hub_pages(
    vault: &crate::Vault,
    request: &TopSitesRequest,
) -> Result<crate::models::IntelligencePage<HubPage>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence,move |connection| {
        let (start_ms,end_ms)=date_range_bounds(connection, &request.date_range)?;
        let limit=request.limit.unwrap_or(10).clamp(1,1000) as usize;
        let after=super::paging::cursor::<(i64,String,String)>(request.cursor.as_deref())?;
        let rows=connection.query(
            "SELECT f.canonical_url,MAX(u.title),f.registrable_domain,COUNT(DISTINCT m.trail_id)
             FROM search_trail_members m
             JOIN archive.visits v ON v.id=m.visit_id
             JOIN archive.urls u ON u.id=v.url_id
             JOIN visit_derived_facts f ON f.visit_id=v.id
             WHERE (?1 IS NULL OR f.profile_id=?1) AND v.visit_time_ms>=?2 AND v.visit_time_ms<?3
             GROUP BY f.canonical_url,f.registrable_domain
             HAVING ?5 IS NULL OR COUNT(DISTINCT m.trail_id)<?5
                OR (COUNT(DISTINCT m.trail_id)=?5 AND (f.canonical_url>?6 OR (f.canonical_url=?6 AND f.registrable_domain>?7)))
             ORDER BY COUNT(DISTINCT m.trail_id) DESC,f.canonical_url,f.registrable_domain LIMIT ?4",
            params![request.profile_id,start_ms,end_ms,(limit+1) as i64,after.as_ref().map(|v|v.0),after.as_ref().map(|v|v.1.as_str()),after.as_ref().map(|v|v.2.as_str())],
            |r| {
                let item=HubPage {url:r.get(0)?,title:r.get(1)?,registrable_domain:r.get(2)?,trail_reference_count:r.get(3)?};
                let cursor=(item.trail_reference_count,item.url.clone(),item.registrable_domain.clone());Ok((item,cursor))
            },
        )?;
        super::paging::finish(rows,limit)
    })
}

/// Loads the canonical visit row needed to start reconstructing a navigation
/// path.
fn load_navigation_visit(connection: &Session<'_>, visit_id: i64) -> Result<Option<VisitRecord>> {
    connection
        .query_one(
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
    connection: &Session<'_>,
    source_profile_id: i64,
    source_visit_id: i64,
) -> Result<Option<VisitRecord>> {
    connection
        .query_one(
            "SELECT visits.id, source_profiles.profile_key, visits.source_profile_id, CAST(visits.source_visit_id AS INTEGER),
                    urls.id, urls.url, urls.title, visits.visit_time_ms, visits.from_visit, visits.transition_type,
                    visits.external_referrer_url
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.source_profile_id = ?1
               AND visits.source_visit_id = CAST(?2 AS TEXT)
             LIMIT 1",
            params![source_profile_id, source_visit_id],
            visit_from_row,
        )
        .optional()
        .map_err(Into::into)
}
