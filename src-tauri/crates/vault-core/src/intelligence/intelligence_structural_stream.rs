//! Failure modes: cursor replay loses the open session or persisted assignments duplicate.
//! Streamed structural replay and persistence for Core Intelligence.
//!
//! ## Responsibilities
//! - Replay structural visit batches into session/trail/search-event tables.
//! - Maintain the in-progress session/trail state machine during one replay
//!   transaction.
//! - Load bounded structural visit batches for one profile scope.
//!
//! ## Not responsible for
//! - Deciding whether structural rebuilds should run incrementally or as a
//!   fallback full refresh.
//! - Building query families, refind pages, or habit aggregates.
//! - Replacing aggregate tables after a rebuild finishes.
//! - Serving route-level read models.
//!
//! ## Dependencies
//! - Shared session/trail build state in `intelligence_structural_state`.
//! - Parent-module structural record types, search-query helpers, and SQLite
//!   row loaders.
//! - `archive.visits`, `search_events`, `search_trails`, and related structural
//!   tables in the intelligence plane.
//!
//! ## Performance notes
//! - The replay path streams visits in ordered batches and only keeps state for
//!   the current session/trail plus prepared SQLite statements.
//! - Range deletes stay scoped to one profile and one dirty window so large
//!   rebuilds do not rewrite unaffected structural rows.

use super::intelligence_structural_state::{
    OpenerWindow, PathSegment, SessionBuildState, TrailBuildState,
};
use super::{
    StructuralTailStreamReport, StructuralVisitRecord, TrailRecord, classify_search_query_kind,
    normalize_query, search_words,
};
use crate::visit_taxonomy::Opener;
use anyhow::Result;
use vault_store::{Session, params_owned as params};

/// Cursor used by streamed structural replay to resume the next ordered visit
/// batch without re-reading prior rows.
/// Prepared statements for streamed structural replay so each visit does not
/// pay repeated SQLite prepare costs.
pub(super) struct StructuralTailPersist<'tx, 'db> {
    /// The zone the days of paths and site flows are in (the build's).
    pub(super) zone: super::InsightsZone,
    generation: i64,
    profile_id: String,
    assignment_statement: vault_store::Statement<'tx, 'db>,
    domain_statement: vault_store::Statement<'tx, 'db>,
    domain_updates: std::collections::BTreeMap<(String, String), i64>,
    flow_statement: vault_store::Statement<'tx, 'db>,
    /// `(day, site, kind, other)` → visits, added to `site_flows` with the domain counts.
    flow_updates: std::collections::BTreeMap<(String, String, &'static str, String), i64>,
    session_statement: vault_store::Statement<'tx, 'db>,
    path_statement: vault_store::Statement<'tx, 'db>,
    trail_statement: vault_store::Statement<'tx, 'db>,
    trail_member_statement: vault_store::Statement<'tx, 'db>,
    search_event_statement: vault_store::Statement<'tx, 'db>,
    search_event_kind_statement: vault_store::Statement<'tx, 'db>,
    search_event_term_delete_statement: vault_store::Statement<'tx, 'db>,
    search_term_statement: vault_store::Statement<'tx, 'db>,
}

impl<'tx, 'db> StructuralTailPersist<'tx, 'db> {
    /// Prepares the structural replay write set inside one transaction.
    pub(super) fn new(
        tx: &'tx Session<'db>,
        generation: i64,
        profile_id: &str,
        zone: super::InsightsZone,
    ) -> Result<Self> {
        Ok(Self {
            zone,
            generation,profile_id:profile_id.to_owned(),
            domain_updates:Default::default(),
            flow_updates: Default::default(),
            flow_statement: tx.prepare(
                "INSERT INTO site_flows_rows(generation,profile_id,date_key,registrable_domain,kind,other,visit_count)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(generation,date_key,profile_id,registrable_domain,kind,other)
                 DO UPDATE SET visit_count=visit_count+excluded.visit_count",
            )?,
            domain_statement:tx.prepare("INSERT INTO session_domains(generation,profile_id,session_id,domain,visit_count) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(generation,session_id,domain) DO UPDATE SET visit_count=visit_count+excluded.visit_count")?,
            assignment_statement: tx.prepare(
                "INSERT OR REPLACE INTO visit_assignments (generation,profile_id,visit_id,session_id,trail_id,ordinal)
                 VALUES (?1,?2,?3,?4,?5,?6)",
            )?,
            session_statement: tx.prepare(
                "INSERT OR REPLACE INTO sessions_rows (generation,session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at) VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?,
            path_statement: tx.prepare(
                "INSERT OR REPLACE INTO session_paths (generation,profile_id,session_id,segment,date_key,prefix_len,sites_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )?,
            trail_statement: tx.prepare(
                "INSERT OR REPLACE INTO search_trails_rows (generation,trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                  landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at) VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?,
            trail_member_statement: tx.prepare(
                "INSERT OR REPLACE INTO search_trail_members_rows (generation,trail_id, profile_id, visit_id, ordinal, role) VALUES (?1,?2, ?3, ?4, ?5, ?6)",
            )?,
            search_event_statement: tx.prepare(
                "INSERT OR REPLACE INTO search_events_rows (generation,visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at, visit_time_ms) VALUES (?1,?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?,
            search_event_kind_statement: tx.prepare(
                "UPDATE search_events_rows SET query_kind=?3 WHERE generation=?1 AND visit_id=?2",
            )?,
            search_event_term_delete_statement: tx.prepare(
                "DELETE FROM search_event_terms_rows WHERE generation=?1 AND visit_id=?2",
            )?,
            search_term_statement: tx.prepare(
                "INSERT OR REPLACE INTO search_event_terms_rows (generation,visit_id, profile_id, term) VALUES (?1,?2, ?3, ?4)",
            )?,
        })
    }

    /// Updates one derived visit assignment with the newly computed
    /// session/trail ids.
    pub(super) fn persist_assignment(
        &mut self,
        visit_id: i64,
        session_id: &str,
        trail_id: Option<&str>,
        ordinal: i64,
        domain: &str,
    ) -> Result<()> {
        self.assignment_statement.execute(params![
            self.generation,
            self.profile_id,
            visit_id,
            session_id,
            trail_id,
            ordinal
        ])?;
        if !domain.is_empty() {
            *self.domain_updates.entry((session_id.to_owned(), domain.to_owned())).or_default() +=
                1;
        }
        Ok(())
    }

    /// Counts one visit toward a site's arrivals or onward sites (`site_flows`) on `day`.
    pub(super) fn count_flow(&mut self, day: &str, site: &str, kind: &'static str, other: &str) {
        *self
            .flow_updates
            .entry((day.to_owned(), site.to_owned(), kind, other.to_owned()))
            .or_default() += 1;
    }

    /// Saves domain and site-flow deltas in the same transaction as assignments and the replay
    /// cursor.
    pub(super) fn flush_counts(&mut self) -> Result<()> {
        for ((session, domain), count) in std::mem::take(&mut self.domain_updates) {
            self.domain_statement.execute(params![
                self.generation,
                self.profile_id,
                session,
                domain,
                count
            ])?;
        }
        for ((day, site, kind, other), count) in std::mem::take(&mut self.flow_updates) {
            self.flow_statement.execute(params![
                self.generation,
                self.profile_id,
                day,
                site,
                kind,
                other,
                count
            ])?;
        }
        Ok(())
    }

    /// Persists one completed session row.
    /// Saves one path segment; the open one is saved again, in place, as it grows.
    pub(super) fn persist_path(&mut self, session_id: &str, path: &PathSegment) -> Result<()> {
        self.path_statement.execute(params![
            self.generation,
            self.profile_id,
            session_id,
            path.segment,
            path.date_key,
            path.prefix_len as i64,
            serde_json::to_string(&path.sites)?,
        ])?;
        Ok(())
    }

    pub(super) fn persist_session(
        &mut self,
        session: &super::SessionRecord,
        computed_at: &str,
    ) -> Result<()> {
        self.session_statement.execute(params![
            self.generation,
            session.session_id,
            session.profile_id,
            session.first_visit_ms,
            session.last_visit_ms,
            session.visit_count,
            session.search_count,
            session.domain_count,
            i64::from(session.is_deep_dive),
            session.auto_title,
            computed_at,
        ])?;
        Ok(())
    }

    /// Persists one completed trail row.
    pub(super) fn persist_trail(&mut self, trail: &TrailRecord, computed_at: &str) -> Result<()> {
        self.trail_statement.execute(params![
            self.generation,
            trail.trail_id,
            trail.profile_id,
            trail.session_id,
            trail.initial_query,
            trail.search_engine,
            trail.reformulation_count,
            trail.visit_count,
            trail.landing_url,
            trail.landing_domain,
            trail.first_visit_ms,
            trail.last_visit_ms,
            trail.max_depth,
            serde_json::to_string(&trail.queries)?,
            computed_at,
        ])?;
        Ok(())
    }

    /// Persists one trail member row.
    pub(super) fn persist_trail_member(&mut self, member: &super::TrailMemberRecord) -> Result<()> {
        self.trail_member_statement.execute(params![
            self.generation,
            member.trail_id,
            member.profile_id,
            member.visit_id,
            member.ordinal,
            member.role,
        ])?;
        Ok(())
    }

    /// Persists the search-event row that seeded a trail and, when needed, its
    /// token rows.
    pub(super) fn persist_search_event(
        &mut self,
        event: &super::SearchEventRecord,
        computed_at: &str,
    ) -> Result<()> {
        self.search_event_statement.execute(params![
            self.generation,
            event.visit_id,
            event.profile_id,
            event.search_engine,
            event.raw_query,
            event.normalized_query,
            event.query_kind.as_str(),
            event.trail_id,
            computed_at,
            event.visit_time_ms,
        ])?;
        if event.query_kind.is_keyword() {
            for term in search_words(&event.normalized_query) {
                self.search_term_statement.execute(params![
                    self.generation,
                    event.visit_id,
                    event.profile_id,
                    term
                ])?;
            }
        }
        Ok(())
    }

    /// Reclassifies the seed search event after the trail's landing domain is
    /// known.
    fn update_search_event_kind(
        &mut self,
        visit_id: i64,
        profile_id: &str,
        normalized_query: &str,
        query_kind: super::SearchQueryKind,
    ) -> Result<()> {
        self.search_event_kind_statement.execute(params![
            self.generation,
            visit_id,
            query_kind.as_str()
        ])?;
        self.search_event_term_delete_statement.execute(params![self.generation, visit_id])?;
        if query_kind.is_keyword() {
            for term in search_words(normalized_query) {
                self.search_term_statement.execute(params![
                    self.generation,
                    visit_id,
                    profile_id,
                    term
                ])?;
            }
        }
        Ok(())
    }
}

/// Holds the current session/trail replay state while structural visits stream
/// through one rebuild transaction.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct StructuralTailStreamState {
    pub(super) current_session: Option<SessionBuildState>,
    pub(super) current_trail: Option<TrailBuildState>,
    pub(super) report: StructuralTailStreamReport,
    #[serde(default)]
    pub(super) openers: OpenerWindow,
}

impl StructuralTailStreamState {
    /// Replays one structural visit into the current session/trail state and
    /// persists any completed boundaries.
    pub(super) fn process_visit(
        &mut self,
        visit: StructuralVisitRecord,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_, '_>,
    ) -> Result<()> {
        self.report.processed_visits += 1;
        self.report.first_visit_ms.get_or_insert(visit.visit_time_ms);

        let starts_new_session = self.current_session.as_ref().is_none_or(|session| {
            visit.visit_time_ms - session.record.last_visit_ms > super::SESSION_GAP_MS
        });
        if starts_new_session {
            self.finish_trail(computed_at, persist)?;
            self.finish_session(computed_at, persist)?;
            self.current_session = Some(SessionBuildState::new(&visit));
        }

        let session = self.current_session.as_mut().expect("structural session");
        session.push_visit(&visit);
        let session_id = session.record.session_id.clone();
        // A path goes from site to site; a visit with no site (an extension page) is not a step.
        if !visit.registrable_domain.is_empty()
            && let Some(closed) =
                session.path.push(persist.zone, &visit.registrable_domain, visit.visit_time_ms)
        {
            persist.persist_path(&session_id, &closed)?;
        }
        let ordinal = session.record.visit_count - 1;
        let mut trail_id = None::<String>;

        if visit.is_search_event {
            let rewords = self.current_trail.as_ref().is_some_and(|trail| {
                trail.record.session_id == session_id
                    && visit.visit_time_ms - trail.record.last_visit_ms <= super::TRAIL_GAP_MS
                    && trail.is_rewording(&visit)
            });
            if !rewords {
                self.finish_trail(computed_at, persist)?;
                self.current_trail = Some(TrailBuildState::new(&visit, &session_id));
            }
            let trail = self.current_trail.as_mut().expect("search trail");
            let member = if rewords {
                trail.append_rewording(&visit)
            } else {
                trail.search_event_member(visit.visit_id)
            };
            let raw_query = visit.search_query.clone().unwrap_or_default();
            let normalized_query = normalize_query(&raw_query);
            let event = super::SearchEventRecord {
                visit_id: visit.visit_id,
                profile_id: visit.profile_id.clone(),
                search_engine: trail.record.search_engine.clone(),
                raw_query: raw_query.clone(),
                normalized_query: normalized_query.clone(),
                query_kind: classify_search_query_kind(&raw_query, &normalized_query, None),
                trail_id: Some(trail.record.trail_id.clone()),
                visit_time_ms: visit.visit_time_ms,
            };
            if event.query_kind.is_keyword() {
                self.report.term_count += search_words(&event.normalized_query).len();
            }
            self.report.member_count += 1;
            persist.persist_search_event(&event, computed_at)?;
            persist.persist_trail_member(&member)?;
            trail_id = Some(trail.record.trail_id.clone());
            trail.record.members.truncate(1);
        } else if let Some(trail) = self.current_trail.as_mut() {
            if trail.record.session_id != session_id
                || visit.visit_time_ms - trail.record.last_visit_ms > super::TRAIL_GAP_MS
            {
                self.finish_trail(computed_at, persist)?;
            } else if trail.relates(&visit) {
                let member = trail.append_visit(&visit);
                trail_id = Some(trail.record.trail_id.clone());
                self.report.member_count += 1;
                persist.persist_trail_member(&member)?;
                trail.record.members.truncate(1);
            } else if trail.pass_unrelated() {
                self.finish_trail(computed_at, persist)?;
            }
        }

        persist.persist_assignment(
            visit.visit_id,
            &session_id,
            trail_id.as_deref(),
            ordinal,
            &visit.registrable_domain,
        )?;
        self.count_flows(&visit, persist);
        Ok(())
    }

    /// How the visit began, by its opener: nothing (typed, a bookmark), a search's results, a
    /// link on another site, or a page PathKeep did not see; the opener's site gains an onward
    /// site. Moving between pages of one site counts for neither. A visit whose browser does not
    /// record openers (Safari, Takeout) counts as unrecorded, so the site page can say it does
    /// not know rather than that nothing came.
    fn count_flows(
        &mut self,
        visit: &StructuralVisitRecord,
        persist: &mut StructuralTailPersist<'_, '_>,
    ) {
        let site = visit.registrable_domain.as_str();
        if !site.is_empty() {
            let day = persist.zone.date_key(visit.visit_time_ms);
            match Opener::of(visit.from_visit) {
                Opener::Unrecorded => persist.count_flow(&day, site, "unrecorded", ""),
                Opener::Nothing => persist.count_flow(&day, site, "direct", ""),
                Opener::Visit(id) => match self.openers.get(id) {
                    Some((opener, _)) if opener == site => {}
                    Some((opener, is_search)) if !opener.is_empty() => {
                        let kind = if *is_search { "search" } else { "link" };
                        persist.count_flow(&day, site, kind, opener);
                        persist.count_flow(&day, opener, "onward", site);
                    }
                    _ => persist.count_flow(&day, site, "link", ""),
                },
            }
        }
        self.openers.push(visit.source_visit_id, site, visit.is_search_event);
    }

    /// Flushes the current trail when a session boundary or replay end is hit.
    fn finish_trail(
        &mut self,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_, '_>,
    ) -> Result<()> {
        if let Some(trail) = self.current_trail.take() {
            if let Some(search_visit_id) =
                trail.record.members.first().map(|member| member.visit_id)
            {
                let normalized_query = normalize_query(&trail.record.initial_query);
                let query_kind = classify_search_query_kind(
                    &trail.record.initial_query,
                    &normalized_query,
                    trail.record.landing_domain.as_deref(),
                );
                let initial = classify_search_query_kind(
                    &trail.record.initial_query,
                    &normalized_query,
                    None,
                );
                let terms = search_words(&normalized_query).len();
                if initial.is_keyword() && !query_kind.is_keyword() {
                    self.report.term_count -= terms;
                }
                if !initial.is_keyword() && query_kind.is_keyword() {
                    self.report.term_count += terms;
                }
                persist.update_search_event_kind(
                    search_visit_id,
                    &trail.record.profile_id,
                    &normalized_query,
                    query_kind,
                )?;
            }
            persist.persist_trail(&trail.finish(), computed_at)?;
            self.report.trails += 1;
        }
        Ok(())
    }

    /// Flushes the current session when a boundary or replay end is hit.
    fn finish_session(
        &mut self,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_, '_>,
    ) -> Result<()> {
        if let Some(session) = self.current_session.take() {
            if session.path.has_own_sites() {
                persist.persist_path(&session.record.session_id, &session.path)?;
            }
            persist.persist_session(&session.finish(), computed_at)?;
            self.report.sessions += 1;
        }
        Ok(())
    }

    /// Finishes any in-progress session/trail after the final batch.
    pub(super) fn finish(
        &mut self,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_, '_>,
    ) -> Result<()> {
        self.finish_trail(computed_at, persist)?;
        self.finish_session(computed_at, persist)
    }
}
