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

use super::intelligence_structural_state::{PathSegment, SessionBuildState, TrailBuildState};
use super::{
    StructuralTailStreamReport, StructuralVisitRecord, TrailRecord, classify_search_query_kind,
    normalize_query, search_words,
};
use anyhow::Result;
use vault_store::{Session, params_owned as params};

/// Cursor used by streamed structural replay to resume the next ordered visit
/// batch without re-reading prior rows.
/// Prepared statements for streamed structural replay so each visit does not
/// pay repeated SQLite prepare costs.
pub(super) struct StructuralTailPersist<'tx, 'db> {
    generation: i64,
    profile_id: String,
    assignment_statement: vault_store::Statement<'tx, 'db>,
    domain_statement: vault_store::Statement<'tx, 'db>,
    domain_updates: std::collections::BTreeMap<(String, String), i64>,
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
    pub(super) fn new(tx: &'tx Session<'db>, generation: i64, profile_id: &str) -> Result<Self> {
        Ok(Self {
            generation,profile_id:profile_id.to_owned(),
            domain_updates:Default::default(),
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

    /// Saves domain deltas in the same transaction as assignments and the replay cursor.
    pub(super) fn flush_domain_counts(&mut self) -> Result<()> {
        for ((session, domain), count) in std::mem::take(&mut self.domain_updates) {
            self.domain_statement.execute(params![
                self.generation,
                self.profile_id,
                session,
                domain,
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
            && let Some(closed) = session.path.push(&visit.registrable_domain, visit.visit_time_ms)
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
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::SearchQueryKind;

    #[test]
    fn search_event_kind_update_rewrites_keyword_terms() {
        let vault = crate::Vault::open_temp().unwrap();
        vault.write(vault_store::Db::Intelligence,vault_store::Priority::Batch,|connection| {
        connection.execute("INSERT INTO intelligence_generations(profile_id,facts,daily,navigation,aggregates) VALUES('chrome:Default',1,1,1,1)",())?;
        connection
            .execute(
                "INSERT INTO search_events_rows
                 (generation,visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at)
                 VALUES (1,42, 'chrome:Default', 'Google', 'GitHub Rust coverage', 'github rust coverage', 'navigational', 'trail-42', '2026-04-26T00:00:00Z')",
                (),
            )
            .expect("insert search event");

        {
            let tx = connection;
            let mut persist = StructuralTailPersist::new(tx,1,"chrome:Default").expect("persist");
            persist
                .update_search_event_kind(
                    42,
                    "chrome:Default",
                    "github rust coverage",
                    SearchQueryKind::Keyword,
                )
                .expect("update to keyword");
            drop(persist);
        }

        let keyword_kind: String = connection
            .query_one("SELECT query_kind FROM search_events WHERE visit_id = 42", (), |row| {
                Ok(row.get(0)?)
            })
            .expect("query kind");
        assert_eq!(keyword_kind, "keyword");

        let keyword_terms = connection
            .prepare("SELECT term FROM search_event_terms WHERE visit_id = 42 ORDER BY term")
            .expect("prepare terms")
            .query_map((), |row| Ok(row.get::< String>(0)?))
            .expect("map terms")
            .collect::<vault_store::Result<Vec<_>>>()
            .expect("collect terms");
        assert_eq!(keyword_terms, vec!["coverage", "github", "rust"]);

        {
            let tx = connection;
            let mut persist = StructuralTailPersist::new(tx,1,"chrome:Default").expect("persist");
            persist
                .update_search_event_kind(
                    42,
                    "chrome:Default",
                    "github rust coverage",
                    SearchQueryKind::Navigational,
                )
                .expect("update to navigational");
            drop(persist);
        }

        let navigational_kind: String = connection
            .query_one("SELECT query_kind FROM search_events WHERE visit_id = 42", (), |row| {
                Ok(row.get(0)?)
            })
            .expect("query kind");
        assert_eq!(navigational_kind, "navigational");

        let term_count: i64 = connection
            .query_one("SELECT COUNT(*) FROM search_event_terms WHERE visit_id = 42", (), |row| {
                Ok(row.get(0)?)
            })
            .expect("term count");
        assert_eq!(term_count, 0);
        Ok(())
        }).unwrap();
    }

    #[test]
    fn stream_state_flushes_trails_on_gap_and_updates_search_event_kind() {
        let vault = crate::Vault::open_temp().unwrap();
        vault.write(vault_store::Db::Intelligence,vault_store::Priority::Batch,|connection| {
        connection.execute("INSERT INTO intelligence_generations(profile_id,facts,daily,navigation,aggregates) VALUES('chrome:Default',1,1,1,1)",())?;
        let computed_at = "2026-04-26T00:00:00Z";
        let tx = connection;
        let mut persist = StructuralTailPersist::new(tx,1,"chrome:Default").expect("persist");
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

        let (query_kind, trail_count): (String, i64) = connection
            .query_one(
                "SELECT search_events.query_kind,
                        (SELECT COUNT(*) FROM search_trails)
                 FROM search_events
                 WHERE search_events.visit_id = 1",
                (),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("persisted search event");
        assert_eq!(query_kind, "keyword");
        assert_eq!(trail_count, 1);
        assert_eq!(state.report.trails, 1);
        assert_eq!(state.report.sessions, 2);
        Ok(())
        }).unwrap();
    }

    /// Searching again for the same thing in more or other words continues the search: one trail
    /// that counts the rewording and ends where the last wording led. Repeating the search is not
    /// a rewording; an unrelated search starts its own trail. Before this, every search started a
    /// trail, so no trail ever counted a rewording.
    #[test]
    fn a_reworded_search_continues_its_trail_and_lands_where_the_last_wording_led() {
        let vault = crate::Vault::open_temp().unwrap();
        vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Batch, |connection| {
                connection.execute("INSERT INTO intelligence_generations(profile_id,facts,daily,navigation,aggregates) VALUES('chrome:Default',1,1,1,1)",())?;
                let mut persist = StructuralTailPersist::new(connection, 1, "chrome:Default")?;
                let mut state = StructuralTailStreamState::default();
                let search = |id: i64, query: &str| StructuralVisitRecord {
                    search_query: Some(query.to_string()),
                    ..structural_visit(id, id * 60_000, "https://www.google.com/search", "google.com", true)
                };
                let click = |id: i64, domain: &str| {
                    structural_visit(id, id * 60_000, &format!("https://{domain}/page"), domain, false)
                };
                for visit in [
                    search(1, "tokio select"),
                    click(2, "docs.rs"),
                    search(3, "tokio select macro"),
                    click(4, "tokio.rs"),
                    search(5, "Tokio  select macro"),
                    search(6, "weather tokyo"),
                    click(7, "weather.example"),
                ] {
                    state.process_visit(visit, "2026-10-08T00:00:00Z", &mut persist)?;
                }
                state.finish("2026-10-08T00:00:00Z", &mut persist)?;
                drop(persist);

                let trails = connection.query(
                    "SELECT trail_id, reformulation_count, queries_json, landing_domain FROM search_trails ORDER BY trail_id",
                    (),
                    |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?, row.get::<String>(2)?, row.get::<Option<String>>(3)?)),
                )?;
                assert_eq!(
                    trails,
                    vec![
                        (
                            "trail:chrome:Default:1".to_string(),
                            1,
                            r#"["tokio select","tokio select macro"]"#.to_string(),
                            Some("tokio.rs".to_string()),
                        ),
                        (
                            "trail:chrome:Default:6".to_string(),
                            0,
                            r#"["weather tokyo"]"#.to_string(),
                            Some("weather.example".to_string()),
                        ),
                    ]
                );
                let events = connection.query(
                    "SELECT visit_id, trail_id FROM search_events ORDER BY visit_id",
                    (),
                    |row| Ok((row.get::<i64>(0)?, row.get::<String>(1)?)),
                )?;
                let first = "trail:chrome:Default:1".to_string();
                assert_eq!(
                    events,
                    vec![
                        (1, first.clone()),
                        (3, first.clone()),
                        (5, first),
                        (6, "trail:chrome:Default:6".to_string()),
                    ]
                );
                Ok(())
            })
            .unwrap();
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
            source_visit_id: visit_id,
            profile_id: "chrome:Default".to_string(),
            url: url.to_string(),
            title: None,
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
