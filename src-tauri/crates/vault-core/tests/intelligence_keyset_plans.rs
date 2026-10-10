//! Responsibilities: prove that later pages of the paged Insights lists seek to their cursor.
//! Not responsible for: page contents, ties or completeness (intelligence_paging covers those).
//! Failure modes: a cursor written as `a<? OR (a=? AND b<?)`, or behind `?N IS NULL OR`, makes
//! Turso walk the index from the first row again on every page (quadratic over a whole list);
//! a read through a generation view sorts the whole generation on every page; or an index that
//! matches the list order goes missing.
//!
//! Each test reads page one, then captures the plans of the real statements while it reads page
//! two with the cursor. Plans do not depend on table contents here (no ANALYZE), so a few rows
//! are enough.
use vault_core::{DateRange, Vault};
use vault_store::{Db, Priority, QueryPlan, params};

const DAY_MS: i64 = 1_767_268_800_000;

fn range() -> DateRange {
    DateRange { start: "2026-01-01".into(), end: "2026-01-02".into() }
}

/// Runs `read` with plan capture on and returns the plans of the statements that use `index`.
fn plans_of<T>(vault: &Vault, db: Db, index: &str, read: impl FnOnce() -> T) -> Vec<QueryPlan> {
    vault.store().capture_query_plans(db, true).unwrap();
    read();
    vault.store().capture_query_plans(db, false).unwrap();
    let plans: Vec<_> = vault
        .store()
        .take_query_plans(db)
        .unwrap()
        .into_iter()
        .filter(|plan| plan.sql.contains(index))
        .collect();
    assert!(!plans.is_empty(), "no statement used {index}");
    plans
}

/// The statements that read a page, leaving out the list's total (`SELECT COUNT`), which reads
/// the same index for the whole window (`intelligence_read_plans` checks it reads no more).
fn pages_only(plans: Vec<QueryPlan>) -> Vec<QueryPlan> {
    let pages: Vec<_> = plans
        .into_iter()
        .filter(|plan| !plan.sql.trim_start().starts_with("SELECT COUNT"))
        .collect();
    assert!(!pages.is_empty(), "no statement read a page");
    pages
}

/// The list's table is searched through `index` with `bounds` bound values, and nothing sorts it
/// or scans it. A cursor the planner cannot seek on shows up as `SCAN` (or as fewer bounds when
/// the window bound alone is used), a read through a view as `USE SORTER FOR ORDER BY`.
fn assert_seeks(plans: &[QueryPlan], alias: &str, index: &str, bounds: usize) {
    for plan in plans {
        let seek = plan.steps.iter().find(|step| {
            step.starts_with(&format!("SEARCH {alias} USING INDEX {index} ("))
                || step.starts_with(&format!("SEARCH {alias} USING COVERING INDEX {index} ("))
        });
        let seek = seek.unwrap_or_else(|| panic!("{index} is not searched: {plan:?}"));
        let constraint = &seek[seek.find('(').unwrap()..];
        assert_eq!(constraint.matches('?').count(), bounds, "{plan:?}");
        assert!(!plan.steps.iter().any(|step| step.contains("SORTER")), "{plan:?}");
        assert!(
            !plan.steps.iter().any(|step| step.starts_with(&format!("SCAN {alias}"))),
            "{plan:?}"
        );
    }
}

/// A list ranked by something other than time (friction, the hardest topics) reads the rows of
/// its range through a time index and sorts them: walked in rank order, a short range read rows
/// until it found a page of its own. Every page sorts the range again, which the range bounds.
/// The table is searched through `index` on at least `bounds` bound values, never scanned.
fn assert_range_sorted(plans: &[QueryPlan], alias: &str, index: &str, bounds: usize) {
    for plan in plans {
        let seek = plan.steps.iter().find(|step| {
            step.starts_with(&format!("SEARCH {alias} USING INDEX {index} ("))
                || step.starts_with(&format!("SEARCH {alias} USING COVERING INDEX {index} ("))
        });
        let seek = seek.unwrap_or_else(|| panic!("{index} is not searched: {plan:?}"));
        let constraint = &seek[seek.find('(').unwrap()..];
        assert!(constraint.matches('?').count() >= bounds, "{plan:?}");
        assert!(
            !plan.steps.iter().any(|step| step.starts_with(&format!("SCAN {alias}"))),
            "{plan:?}"
        );
    }
}

fn seed_intelligence(vault: &Vault) {
    vault.store().write(Db::Archive, Priority::Batch, |s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Profile','/fixture','profile','')", ())?;
        Ok(())
    }).unwrap();
    vault.store().write(Db::Intelligence, Priority::Batch, |s| {
        s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile')", ())?;
        for id in 1..=5_i64 {
            s.execute("INSERT INTO sessions_rows(session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,computed_at) VALUES(?1,'profile',?2,?2,1,1,1,'')", params![format!("session:{id}"), DAY_MS])?;
            s.execute("INSERT INTO search_trails_rows(trail_id,profile_id,session_id,initial_query,search_engine,reformulation_count,visit_count,first_visit_ms,last_visit_ms,queries_json,computed_at) VALUES(?1,'profile','session:1','query','google',2,3,?2,?2,'[]','')", params![format!("trail:{id}"), DAY_MS])?;
            s.execute("INSERT INTO query_families_rows(family_id,profile_id,anchor_query,member_count,search_engine,first_seen_ms,last_seen_ms,queries_json,computed_at) VALUES(?1,'profile','query',2,'google',?2,?2,'[]','')", params![format!("family:{id}"), DAY_MS])?;
            s.execute("INSERT INTO reopened_investigations_rows(investigation_id,profile_id,anchor_type,anchor_id,anchor_label,occurrence_count,distinct_days,first_seen_ms,last_seen_ms,evidence_json,computed_at) VALUES(?1,'profile','query_family',?1,?1,3,2,?2,?2,'{}','')", params![format!("investigation:{id}"), DAY_MS])?;
            s.execute("INSERT INTO refind_pages_rows(profile_id,canonical_url,url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms,computed_at) VALUES('profile',?1,?1,'Page','example.com',3,2,2,2,1.0,'{}',?2,?2,'')", params![format!("https://example.com/{id}"), DAY_MS])?;
            s.execute("INSERT INTO path_flows_rows(profile_id,flow_pattern,step_count,occurrence_count,last_seen_ms) VALUES('profile',?1,2,2,?2)", params![format!("site{id}.test → example.com"), DAY_MS])?;
        }
        Ok(())
    }).unwrap();
}

#[test]
fn session_trail_and_family_pages_seek_to_their_cursor() {
    let vault = Vault::open_temp().unwrap();
    seed_intelligence(&vault);
    for profile in [None, Some("profile".to_owned())] {
        let sessions = |cursor| {
            vault_core::get_sessions(
                &vault,
                &vault_core::PagedDateRangeRequest {
                    date_range: range(),
                    profile_id: profile.clone(),
                    page_size: 2,
                    cursor,
                },
            )
            .unwrap()
        };
        let cursor = sessions(None).next_cursor;
        assert!(cursor.is_some());
        let plans = plans_of(&vault, Db::Intelligence, "idx_sessions_recent", || sessions(cursor));
        assert_seeks(&plans, "r", "idx_sessions_recent", 1);

        let trails = |cursor| {
            vault_core::get_search_trails(
                &vault,
                &vault_core::SearchTrailQueryRequest {
                    date_range: range(),
                    profile_id: profile.clone(),
                    engine: None,
                    page_size: 2,
                    cursor,
                },
            )
            .unwrap()
        };
        let cursor = trails(None).next_cursor;
        assert!(cursor.is_some());
        let plans =
            plans_of(&vault, Db::Intelligence, "idx_search_trails_recent", || trails(cursor));
        // The window's start and the cursor both bound first_visit_ms.
        assert_seeks(&pages_only(plans), "r", "idx_search_trails_recent", 2);

        let families = |cursor| {
            vault_core::get_query_families(
                &vault,
                &vault_core::PagedDateRangeRequest {
                    date_range: range(),
                    profile_id: profile.clone(),
                    page_size: 2,
                    cursor,
                },
            )
            .unwrap()
        };
        let cursor = families(None).next_cursor;
        assert!(cursor.is_some());
        let plans =
            plans_of(&vault, Db::Intelligence, "idx_query_families_recent", || families(cursor));
        // The window's start and the cursor both bound last_seen_ms.
        assert_seeks(&pages_only(plans), "r", "idx_query_families_recent", 2);
    }
}

#[test]
fn ranked_summary_pages_seek_to_their_cursor() {
    let vault = Vault::open_temp().unwrap();
    seed_intelligence(&vault);
    for profile in [None, Some("profile".to_owned())] {
        let request = |cursor| vault_core::ScopedPageRequest {
            date_range: range(),
            profile_id: profile.clone(),
            limit: Some(2),
            cursor,
        };

        let cursor = vault_core::get_friction_signals(&vault, &request(None)).unwrap().next_cursor;
        assert!(cursor.is_some());
        let plans = plans_of(&vault, Db::Intelligence, "idx_search_trails_friction", || {
            vault_core::get_friction_signals(&vault, &request(cursor)).unwrap()
        });
        // The trails begun in the window, between its two bounds.
        assert_range_sorted(&plans, "r", "idx_search_trails_friction", 2);

        let cursor =
            vault_core::get_reopened_investigations(&vault, &request(None)).unwrap().next_cursor;
        assert!(cursor.is_some());
        let plans =
            plans_of(&vault, Db::Intelligence, "idx_reopened_investigations_recent", || {
                vault_core::get_reopened_investigations(&vault, &request(cursor)).unwrap()
            });
        // The window's start and the cursor both bound last_seen_ms.
        assert_seeks(&plans, "r", "idx_reopened_investigations_recent", 2);

        let topics = |cursor| {
            vault_core::get_search_effectiveness(
                &vault,
                &vault_core::SearchEffectivenessRequest {
                    date_range: range(),
                    profile_id: profile.clone(),
                    engine: None,
                    topics_limit: Some(2),
                    topics_cursor: cursor,
                },
            )
            .unwrap()
        };
        let cursor = topics(None).hardest_topics.next_cursor;
        assert!(cursor.is_some());
        let plans =
            plans_of(&vault, Db::Intelligence, "idx_query_families_recent", || topics(cursor));
        assert_range_sorted(&plans, "r", "idx_query_families_recent", 1);

        let refind = |cursor| {
            vault_core::get_refind_pages(
                &vault,
                &vault_core::RefindPagesRequest {
                    date_range: range(),
                    profile_id: profile.clone(),
                    limit: Some(2),
                    cursor,
                },
            )
            .unwrap()
        };
        let cursor = refind(None).next_cursor;
        assert!(cursor.is_some());
        let plans = plans_of(&vault, Db::Intelligence, "idx_refind_pages_score", || refind(cursor));
        assert_seeks(&plans, "p", "idx_refind_pages_score", 1);
        // The best-profile lookups for each row seek by URL instead of indexing the whole table.
        for plan in &plans {
            assert!(!plan.steps.iter().any(|step| step.contains("ephemeral")), "{plan:?}");
        }
    }
}

#[test]
fn one_profiles_all_time_flows_page_through_their_rows_in_index_order() {
    let vault = Vault::open_temp().unwrap();
    seed_intelligence(&vault);
    let flows = |cursor| {
        vault_core::get_path_flows(
            &vault,
            &vault_core::PathFlowRequest {
                date_range: DateRange { start: "1900-01-01".into(), end: "2100-01-01".into() },
                profile_id: Some("profile".into()),
                step_count: 2,
                limit: Some(2),
                cursor,
            },
        )
        .unwrap()
    };
    let cursor = flows(None).next_cursor;
    assert!(cursor.is_some());
    let plans = plans_of(&vault, Db::Intelligence, "idx_path_flows_profile_rank", || flows(cursor));
    // Profile, step count and the cursor's occurrence count.
    assert_seeks(&plans, "r", "idx_path_flows_profile_rank", 3);
}

#[test]
fn observed_interaction_pages_seek_the_visit_time_index_to_the_cursor() {
    let vault = Vault::open_temp().unwrap();
    vault.store().write(Db::Archive, Priority::Batch, |s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')", ())?;
        s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,'https://example.com/','https://example.com/')", ())?;
        s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,?1,'',?1,'')", [DAY_MS])?;
        for id in 1..=5_i64 {
            s.execute("INSERT INTO visits(id,url_id,source_visit_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,1,?2,1,0,?3,'')", params![id, id.to_string(), DAY_MS])?;
        }
        Ok(())
    }).unwrap();
    vault.store().write(Db::SourceEvidence, Priority::Batch, |s| {
        s.execute("INSERT INTO visit_engagement_evidence(source_batch_id,source_profile_id,source_visit_id,metric_key,metric_value_int,source_field,recorded_at) VALUES(0,1,'1','engagement.total_foreground_duration_ms',42,'fixture','')", ())?;
        Ok(())
    }).unwrap();
    let page = |cursor| {
        vault_core::get_observed_interactions(
            &vault,
            &vault_core::ScopedPageRequest {
                date_range: range(),
                profile_id: None,
                limit: Some(2),
                cursor,
            },
        )
        .unwrap()
    };
    let cursor = page(None).next_cursor;
    assert!(cursor.is_some());
    let plans = plans_of(&vault, Db::Intelligence, "idx_visits_visible_time_id", || page(cursor));
    assert_seeks(&plans, "v", "idx_visits_visible_time_id", 2);
    // The upper bound is the cursor (`<=`), not the window's end (`<`).
    for plan in &plans {
        assert!(plan.steps.iter().any(|step| step.contains("visit_time_ms<=?")), "{plan:?}");
    }
}
