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
//! - Dirty-range cleanup helpers in `intelligence_structural_persist`.
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

use super::intelligence_structural_persist::clear_structural_tail_state;
use super::intelligence_structural_state::{SessionBuildState, TrailBuildState};
use super::{
    StructuralTailStreamReport, StructuralVisitRecord, TrailRecord, classify_search_query_kind,
    load_archive_source_profile_id, normalize_query, tokenize_query_terms,
};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::BTreeSet;

/// Cursor used by streamed structural replay to resume the next ordered visit
/// batch without re-reading prior rows.
#[derive(Debug, Clone, Copy)]
struct StructuralVisitBatchCursor {
    visit_time_ms: i64,
    visit_id: i64,
}

/// Prepared statements for streamed structural replay so each visit does not
/// pay repeated SQLite prepare costs.
struct StructuralTailPersist<'tx> {
    assignment_statement: rusqlite::Statement<'tx>,
    session_statement: rusqlite::Statement<'tx>,
    trail_statement: rusqlite::Statement<'tx>,
    trail_member_statement: rusqlite::Statement<'tx>,
    search_event_statement: rusqlite::Statement<'tx>,
    search_event_kind_statement: rusqlite::Statement<'tx>,
    search_event_term_delete_statement: rusqlite::Statement<'tx>,
    search_term_statement: rusqlite::Statement<'tx>,
}

impl<'tx> StructuralTailPersist<'tx> {
    /// Prepares the structural replay write set inside one transaction.
    fn new(tx: &'tx rusqlite::Transaction<'tx>) -> Result<Self> {
        Ok(Self {
            assignment_statement: tx.prepare(
                "UPDATE visit_derived_facts
                 SET session_id = ?2, trail_id = ?3, computed_at = ?4
                 WHERE visit_id = ?1",
            )?,
            session_statement: tx.prepare(
                "INSERT INTO sessions
                 (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?,
            trail_statement: tx.prepare(
                "INSERT INTO search_trails
                 (trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                  landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?,
            trail_member_statement: tx.prepare(
                "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?,
            search_event_statement: tx.prepare(
                "INSERT INTO search_events
                 (visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?,
            search_event_kind_statement: tx.prepare(
                "UPDATE search_events SET query_kind = ?2 WHERE visit_id = ?1",
            )?,
            search_event_term_delete_statement: tx.prepare(
                "DELETE FROM search_event_terms WHERE visit_id = ?1",
            )?,
            search_term_statement: tx.prepare(
                "INSERT INTO search_event_terms (visit_id, profile_id, term)
                 VALUES (?1, ?2, ?3)",
            )?,
        })
    }

    /// Updates one derived visit assignment with the newly computed
    /// session/trail ids.
    fn persist_assignment(
        &mut self,
        visit_id: i64,
        session_id: &str,
        trail_id: Option<&str>,
        computed_at: &str,
    ) -> Result<()> {
        self.assignment_statement.execute(params![visit_id, session_id, trail_id, computed_at])?;
        Ok(())
    }

    /// Persists one completed session row.
    fn persist_session(&mut self, session: &super::SessionRecord, computed_at: &str) -> Result<()> {
        self.session_statement.execute(params![
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
    fn persist_trail(&mut self, trail: &TrailRecord, computed_at: &str) -> Result<()> {
        self.trail_statement.execute(params![
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
    fn persist_trail_member(&mut self, member: &super::TrailMemberRecord) -> Result<()> {
        self.trail_member_statement.execute(params![
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
    fn persist_search_event(
        &mut self,
        event: &super::SearchEventRecord,
        computed_at: &str,
    ) -> Result<()> {
        self.search_event_statement.execute(params![
            event.visit_id,
            event.profile_id,
            event.search_engine,
            event.raw_query,
            event.normalized_query,
            event.query_kind.as_str(),
            event.trail_id,
            computed_at,
        ])?;
        if event.query_kind.is_keyword() {
            for term in
                tokenize_query_terms(&event.normalized_query).into_iter().collect::<BTreeSet<_>>()
            {
                self.search_term_statement.execute(params![
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
        self.search_event_kind_statement.execute(params![visit_id, query_kind.as_str()])?;
        self.search_event_term_delete_statement.execute([visit_id])?;
        if query_kind.is_keyword() {
            for term in tokenize_query_terms(normalized_query).into_iter().collect::<BTreeSet<_>>()
            {
                self.search_term_statement.execute(params![visit_id, profile_id, term])?;
            }
        }
        Ok(())
    }
}

/// Holds the current session/trail replay state while structural visits stream
/// through one rebuild transaction.
#[derive(Default)]
struct StructuralTailStreamState {
    current_session: Option<SessionBuildState>,
    current_trail: Option<TrailBuildState>,
    report: StructuralTailStreamReport,
}

impl StructuralTailStreamState {
    /// Replays one structural visit into the current session/trail state and
    /// persists any completed boundaries.
    fn process_visit(
        &mut self,
        visit: StructuralVisitRecord,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_>,
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
        let mut trail_id = None::<String>;

        if visit.is_search_event {
            self.finish_trail(computed_at, persist)?;
            let trail = TrailBuildState::new(&visit, &session_id);
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
            persist.persist_search_event(&event, computed_at)?;
            persist.persist_trail_member(&trail.search_event_member(visit.visit_id))?;
            trail_id = Some(trail.record.trail_id.clone());
            self.current_trail = Some(trail);
        } else if let Some(trail) = self.current_trail.as_mut() {
            if trail.record.session_id != session_id
                || visit.visit_time_ms - trail.record.last_visit_ms > super::TRAIL_GAP_MS
            {
                self.finish_trail(computed_at, persist)?;
            } else {
                let member = trail.append_visit(&visit);
                trail_id = Some(trail.record.trail_id.clone());
                persist.persist_trail_member(&member)?;
            }
        }

        persist.persist_assignment(
            visit.visit_id,
            &session_id,
            trail_id.as_deref(),
            computed_at,
        )?;
        Ok(())
    }

    /// Flushes the current trail when a session boundary or replay end is hit.
    fn finish_trail(
        &mut self,
        computed_at: &str,
        persist: &mut StructuralTailPersist<'_>,
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
        persist: &mut StructuralTailPersist<'_>,
    ) -> Result<()> {
        if let Some(session) = self.current_session.take() {
            persist.persist_session(&session.finish(), computed_at)?;
            self.report.sessions += 1;
        }
        Ok(())
    }

    /// Finishes any in-progress session/trail after the final batch.
    fn finish(&mut self, computed_at: &str, persist: &mut StructuralTailPersist<'_>) -> Result<()> {
        self.finish_trail(computed_at, persist)?;
        self.finish_session(computed_at, persist)
    }
}

/// Streams structural visits from `visit_derived_facts` in chronological order
/// for one profile scope.
fn load_structural_visit_batch(
    connection: &Connection,
    profile_id: &str,
    source_profile_id: i64,
    start_ms: Option<i64>,
    after: Option<StructuralVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<StructuralVisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.visit_id,
                ?1,
                urls.url,
                visits.visit_time_ms,
                visits.from_visit,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event
         FROM archive.visits AS visits
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visits.reverted_at IS NULL
           AND visits.source_profile_id = ?2
           AND (?3 IS NULL OR visits.visit_time_ms >= ?3)
           AND (
             ?4 IS NULL
             OR visits.visit_time_ms > ?4
             OR (visits.visit_time_ms = ?4 AND visits.id > ?5)
           )
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?6",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                source_profile_id,
                start_ms,
                after.map(|cursor| cursor.visit_time_ms),
                after.map(|cursor| cursor.visit_id),
                limit.max(1) as i64,
            ],
            |row| {
                Ok(StructuralVisitRecord {
                    visit_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    url: row.get(2)?,
                    visit_time_ms: row.get(3)?,
                    from_visit: row.get(4)?,
                    registrable_domain: row.get(5)?,
                    search_engine: row.get(6)?,
                    search_query: row.get(7)?,
                    is_new_domain: row.get::<_, i64>(8)? != 0,
                    is_search_event: row.get::<_, i64>(9)? != 0,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Replays one profile's structural tail in chronological batches and persists
/// the rebuilt session/trail/search-event rows in one transaction.
pub(super) fn rebuild_structural_tail_state(
    connection: &Connection,
    profile_id: &str,
    start_ms: Option<i64>,
    computed_at: &str,
    batch_size: usize,
) -> Result<StructuralTailStreamReport> {
    let source_profile_id = load_archive_source_profile_id(connection, profile_id)?;
    let tx = connection.unchecked_transaction()?;
    clear_structural_tail_state(&tx, profile_id, start_ms)?;
    let mut persist = StructuralTailPersist::new(&tx)?;
    let mut state = StructuralTailStreamState::default();
    let mut cursor = None::<StructuralVisitBatchCursor>;

    loop {
        let batch = load_structural_visit_batch(
            &tx,
            profile_id,
            source_profile_id,
            start_ms,
            cursor,
            batch_size,
        )?;
        if batch.is_empty() {
            break;
        }
        let next_cursor = batch.last().map(|visit| StructuralVisitBatchCursor {
            visit_time_ms: visit.visit_time_ms,
            visit_id: visit.visit_id,
        });
        for visit in batch {
            state.process_visit(visit, computed_at, &mut persist)?;
        }
        cursor = next_cursor;
    }

    state.finish(computed_at, &mut persist)?;
    drop(persist);
    tx.commit()?;
    Ok(state.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::{SearchQueryKind, ensure_core_intelligence_schema};

    #[test]
    fn search_event_kind_update_rewrites_keyword_terms() {
        let connection = Connection::open_in_memory().expect("open intelligence connection");
        ensure_core_intelligence_schema(&connection).expect("schema");
        connection
            .execute(
                "INSERT INTO search_events
                 (visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at)
                 VALUES (42, 'chrome:Default', 'Google', 'GitHub Rust coverage', 'github rust coverage', 'navigational', 'trail-42', '2026-04-26T00:00:00Z')",
                [],
            )
            .expect("insert search event");

        {
            let tx = connection.unchecked_transaction().expect("transaction");
            let mut persist = StructuralTailPersist::new(&tx).expect("persist");
            persist
                .update_search_event_kind(
                    42,
                    "chrome:Default",
                    "github rust coverage",
                    SearchQueryKind::Keyword,
                )
                .expect("update to keyword");
            drop(persist);
            tx.commit().expect("commit keyword update");
        }

        let keyword_kind: String = connection
            .query_row("SELECT query_kind FROM search_events WHERE visit_id = 42", [], |row| {
                row.get(0)
            })
            .expect("query kind");
        assert_eq!(keyword_kind, "keyword");

        let keyword_terms = connection
            .prepare("SELECT term FROM search_event_terms WHERE visit_id = 42 ORDER BY term")
            .expect("prepare terms")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("map terms")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect terms");
        assert_eq!(keyword_terms, vec!["coverage", "github", "rust"]);

        {
            let tx = connection.unchecked_transaction().expect("transaction");
            let mut persist = StructuralTailPersist::new(&tx).expect("persist");
            persist
                .update_search_event_kind(
                    42,
                    "chrome:Default",
                    "github rust coverage",
                    SearchQueryKind::Navigational,
                )
                .expect("update to navigational");
            drop(persist);
            tx.commit().expect("commit navigational update");
        }

        let navigational_kind: String = connection
            .query_row("SELECT query_kind FROM search_events WHERE visit_id = 42", [], |row| {
                row.get(0)
            })
            .expect("query kind");
        assert_eq!(navigational_kind, "navigational");

        let term_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM search_event_terms WHERE visit_id = 42", [], |row| {
                row.get(0)
            })
            .expect("term count");
        assert_eq!(term_count, 0);
    }

    #[test]
    fn stream_state_flushes_trails_on_gap_and_updates_search_event_kind() {
        let connection = Connection::open_in_memory().expect("open intelligence connection");
        ensure_core_intelligence_schema(&connection).expect("schema");
        let computed_at = "2026-04-26T00:00:00Z";
        let tx = connection.unchecked_transaction().expect("transaction");
        let mut persist = StructuralTailPersist::new(&tx).expect("persist");
        let mut state = StructuralTailStreamState::default();

        state
            .process_visit(
                structural_visit(
                    1,
                    1_000,
                    "https://www.google.com/search?q=pathkeep+sqlite",
                    "google.com",
                    true,
                ),
                computed_at,
                &mut persist,
            )
            .expect("search visit");
        state
            .process_visit(
                structural_visit(2, 2_000, "https://github.com/pathkeep/repo", "github.com", false),
                computed_at,
                &mut persist,
            )
            .expect("landing visit");
        state
            .process_visit(
                structural_visit(3, 1_000_000, "https://example.com/later", "example.com", false),
                computed_at,
                &mut persist,
            )
            .expect("trail gap visit");
        state
            .process_visit(
                structural_visit(
                    4,
                    3_600_000,
                    "https://example.com/new-session",
                    "example.com",
                    false,
                ),
                computed_at,
                &mut persist,
            )
            .expect("session gap visit");
        state.finish(computed_at, &mut persist).expect("finish stream");
        drop(persist);
        tx.commit().expect("commit structural stream");

        let (query_kind, trail_count): (String, i64) = connection
            .query_row(
                "SELECT search_events.query_kind,
                        (SELECT COUNT(*) FROM search_trails)
                 FROM search_events
                 WHERE search_events.visit_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("persisted search event");
        assert_eq!(query_kind, "keyword");
        assert_eq!(trail_count, 1);
        assert_eq!(state.report.trails, 1);
        assert_eq!(state.report.sessions, 2);
    }

    fn structural_visit(
        visit_id: i64,
        visit_time_ms: i64,
        url: &str,
        registrable_domain: &str,
        is_search_event: bool,
    ) -> StructuralVisitRecord {
        StructuralVisitRecord {
            visit_id,
            profile_id: "chrome:Default".to_string(),
            url: url.to_string(),
            visit_time_ms,
            from_visit: (visit_id > 1).then_some(visit_id - 1),
            registrable_domain: registrable_domain.to_string(),
            search_engine: is_search_event.then(|| "google".to_string()),
            search_query: is_search_event.then(|| "pathkeep sqlite".to_string()),
            is_new_domain: visit_id == 1,
            is_search_event,
        }
    }
}
