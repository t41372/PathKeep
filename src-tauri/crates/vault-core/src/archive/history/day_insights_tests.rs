//! Responsibilities: verify full-day counts, transitions, sessions, dates and index seeks.
//! Not responsible for: intelligence projections.
//! Failure modes: scroll-dependent totals, profile leakage, reverted visits, DST gaps, full scans.
use super::*;
use vault_store::{Db, Priority, params};

fn fresh_vault(key: Option<[u8; 32]>) -> Vault {
    let vault = Vault::open_temp_with_key(key).unwrap();
    vault.store().write(Db::Archive, Priority::Batch, |s| {
            s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES (1,'chrome:Default','chromium','Default','/chrome','2026-05-25'),(2,'firefox:Default','firefox','Default','/firefox','2026-05-25')",())?;
            Ok(())
        }).unwrap();
    vault
}

fn insert_visit(
    vault: &Vault,
    profile: i64,
    url: &str,
    title: Option<&str>,
    time: i64,
    transition: i64,
) {
    let (url, title) = (url.to_owned(), title.map(str::to_owned));
    vault.store().write(Db::Archive, Priority::Batch, move |s| {
            s.execute("INSERT INTO pages(url,canonical_url) VALUES(?1,?1) ON CONFLICT(url) DO NOTHING", [url.as_str()])?;
            let page: i64 = s.query_one("SELECT id FROM pages WHERE url=?1", [url.as_str()], |r| Ok(r.get(0)?))?;
            let id = s.query("SELECT id FROM urls WHERE url=?1 AND source_profile_id=?2", params![url.as_str(),profile], |r| Ok(r.get::<i64>(0)?))?.pop();
            let id = match id {
                Some(id) => id,
                None => s.query_one("INSERT INTO urls(page_id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) VALUES(?1,?2,?3,1,0,?4,'',?4,'',?5,0) RETURNING id", params![page,url,title,time,profile], |r| Ok(r.get::<i64>(0)?))?,
            };
            s.execute("INSERT INTO visits(url_id,visit_time_ms,visit_time_iso,transition_type,source_profile_id,created_by_run_id) VALUES(?1,?2,'',?3,?4,0)", params![id,time,transition,profile])?;
            Ok(())
        }).unwrap();
}

// Helper: a fixed local-day timestamp so tests don't depend on the
// host clock. We resolve "2026-05-25 HH:MM:SS local" into a single
// millis value.
//
// DST handling: if the requested local time falls into a DST
// "spring-forward" gap (Ambiguous / None on either call), retry one
// hour later. The retry is wrapped in an explicit `match` instead
// of `.unwrap()` so a hypothetical future tzdata with a >2-hour
// jump that nukes both midnight AND 01:00 fails with a clear
// assertion message — the previous `.unwrap()` would emit a bare
// "called `Option::unwrap()` on a `None` value" with no file:line
// context inside the test runner. Claude review finding #8.
fn local_ms(hour: u32, minute: u32, second: u32) -> i64 {
    let day = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
    let first = Local.from_local_datetime(&day.and_hms_opt(hour, minute, second).unwrap()).single();
    let resolved = match first {
        Some(dt) => dt,
        None => match Local
            .from_local_datetime(&day.and_hms_opt(hour + 1, minute, second).unwrap())
            .single()
        {
            Some(dt) => dt,
            None => panic!(
                "DST helper exhausted at 2026-05-25 {hour:02}:{minute:02}:{second:02} \
                     local — both the requested hour and hour+1 returned Ambiguous/None. \
                     The test tzdata likely has a >2h DST jump or the helper needs widening.",
            ),
        },
    };
    resolved.timestamp_millis()
}

#[test]
fn empty_day_returns_zero_metrics_with_padded_hour_buckets() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        let insights = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 0);
        assert_eq!(insights.session_count, 0);
        assert_eq!(insights.hour_buckets.len(), 24);
        assert_eq!(insights.hour_peak, 1);
        assert!(insights.peak_hour.is_none());
        assert!(insights.first_visit_ms.is_none());
        assert!(insights.last_visit_ms.is_none());
        assert!(insights.top_domains.is_empty());
        assert!(insights.top_urls.is_empty());
        assert!(insights.top_search_queries.is_empty());
    }
}

#[test]
fn aggregates_transitions_domains_urls_and_search_queries() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        // Three visits to one domain (one typed, two link), one search
        // visit, and one extra page on a second domain.
        insert_visit(
            &connection,
            1,
            "https://news.example.com/a",
            Some("A"),
            local_ms(9, 0, 0),
            TRANSITION_TYPED,
        );
        insert_visit(
            &connection,
            1,
            "https://news.example.com/b",
            Some("B"),
            local_ms(9, 5, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            1,
            "https://news.example.com/a",
            Some("A"),
            local_ms(9, 8, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            1,
            "https://www.google.com/search?q=sqlite+wal",
            Some("Google"),
            local_ms(10, 0, 0),
            TRANSITION_GENERATED,
        );
        insert_visit(
            &connection,
            1,
            "https://docs.example.org/foo",
            None,
            local_ms(11, 0, 0),
            TRANSITION_LINK,
        );

        let insights = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 5);
        assert_eq!(insights.typed_count, 1);
        assert_eq!(insights.link_count, 3);
        assert_eq!(insights.search_count, 1);
        assert_eq!(insights.distinct_domains, 3);

        // The most-visited URL is /a with 2 visits, then /b, /search, /foo.
        assert_eq!(insights.top_urls[0].url, "https://news.example.com/a");
        assert_eq!(insights.top_urls[0].visits, 2);

        // news.example.com leads with 3 visits.
        assert_eq!(insights.top_domains[0].domain, "news.example.com");
        assert_eq!(insights.top_domains[0].visits, 3);

        // Search query extracted from google.com.
        assert_eq!(insights.top_search_queries.len(), 1);
        assert_eq!(insights.top_search_queries[0].query, "sqlite wal");
        assert_eq!(insights.top_search_queries[0].count, 1);

        assert_eq!(insights.first_visit_ms, Some(local_ms(9, 0, 0)));
        assert_eq!(insights.last_visit_ms, Some(local_ms(11, 0, 0)));
    }
}

#[test]
fn session_walk_splits_on_gap_greater_than_thirty_minutes() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        // Session 1: 09:00, 09:15 (15-min gap), 09:30 (15-min gap)
        // Session 2: 11:00 (gap of 1.5h splits)
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 15, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 30, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(11, 0, 0), TRANSITION_LINK);

        let insights = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.session_count, 2);
        // Longest session is session 1 spanning 30 minutes = 1_800_000 ms.
        assert_eq!(insights.longest_session_ms, 30 * 60 * 1000);
    }
}

#[test]
fn hour_buckets_track_local_hour_and_peak_hour_picks_the_busiest() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(14, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(14, 30, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(14, 45, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(20, 0, 0), TRANSITION_LINK);

        let insights = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.hour_buckets.len(), 24);
        assert_eq!(insights.hour_buckets[9], 1);
        assert_eq!(insights.hour_buckets[14], 3);
        assert_eq!(insights.hour_buckets[20], 1);
        assert_eq!(insights.peak_hour, Some(14));
        assert_eq!(insights.hour_peak, 3);
    }
}

#[test]
fn profile_filter_only_aggregates_the_requested_profile() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        insert_visit(
            &connection,
            1,
            "https://chrome.test/",
            None,
            local_ms(9, 0, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            1,
            "https://chrome.test/",
            None,
            local_ms(9, 1, 0),
            TRANSITION_LINK,
        );
        insert_visit(
            &connection,
            2,
            "https://firefox.test/",
            None,
            local_ms(10, 0, 0),
            TRANSITION_LINK,
        );
        // Aggregate across both:
        let all = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(all.total_pages, 3);
        // Filter to chrome only:
        let chrome = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest {
                date: "2026-05-25".to_string(),
                profile_id: Some("chrome:Default".to_string()),
            },
        )
        .unwrap();
        assert_eq!(chrome.total_pages, 2);
        assert_eq!(chrome.distinct_domains, 1);
        assert_eq!(chrome.top_domains[0].domain, "chrome.test");
    }
}

#[test]
fn reverted_visits_are_excluded_from_the_aggregate() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 0, 0), TRANSITION_LINK);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(9, 5, 0), TRANSITION_LINK);
        // Mark the second visit reverted.
        connection
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                s.execute(
                "UPDATE visits SET reverted_at = '2026-05-25T00:00:00Z' WHERE visit_time_ms = ?1",
                params![local_ms(9, 5, 0)],
            )?;
                Ok(())
            })
            .unwrap();
        let insights = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 1);
    }
}

#[test]
fn invalid_date_string_returns_an_error() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        let result = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "not-a-date".to_string(), profile_id: None },
        );
        assert!(result.is_err());
    }
}

#[test]
fn extract_search_query_understands_known_engines_only() {
    assert_eq!(
        extract_search_query("https://www.google.com/search?q=sqlite+wal"),
        Some("sqlite wal".to_string()),
    );
    assert_eq!(
        extract_search_query("https://duckduckgo.com/?q=pathkeep"),
        Some("pathkeep".to_string()),
    );
    // Yandex uses `text`, not `q`.
    assert_eq!(extract_search_query("https://yandex.ru/?q=ignored"), None);
    assert_eq!(
        extract_search_query("https://yandex.ru/?text=pathkeep"),
        Some("pathkeep".to_string()),
    );
    // Empty values are rejected.
    assert_eq!(extract_search_query("https://www.google.com/search?q="), None);
    // Very long pastes are rejected.
    let long = "a".repeat(MAX_SEARCH_QUERY_LEN + 1);
    let url = format!("https://www.google.com/search?q={long}");
    assert_eq!(extract_search_query(&url), None);
    // Unknown hosts contribute nothing.
    assert_eq!(extract_search_query("https://example.com/?q=foo"), None);
}

#[test]
fn get_browse_day_insights_public_api_opens_archive_and_aggregates_the_requested_day() {
    for key in [None, Some([73; 32])] {
        let connection = fresh_vault(key);
        insert_visit(&connection, 1, "https://a.test/", None, local_ms(10, 0, 0), TRANSITION_LINK);

        let insights = get_browse_day_insights(
            &connection,
            &BrowseDayInsightsRequest { date: "2026-05-25".to_string(), profile_id: None },
        )
        .expect("public API call should succeed");
        assert_eq!(insights.total_pages, 1);
        assert_eq!(insights.distinct_domains, 1);
    }
}

#[test]
fn compute_browse_day_insights_back_fills_title_when_first_visit_was_titleless() {
    // Two visits to the same URL: the first carries no title, the
    // second carries one. The aggregator must back-fill so the
    // top-URLs row in the day-insights panel doesn't render with
    // an empty title when a later visit did capture it.
    let visits = vec![
        DayVisitRow {
            id: 0,
            visit_time_ms: 1_700_000_000_000,
            transition_type: Some(0),
            url: "https://example.test/article".to_string(),
            title: None,
        },
        DayVisitRow {
            id: 0,
            visit_time_ms: 1_700_000_060_000,
            transition_type: Some(0),
            url: "https://example.test/article".to_string(),
            title: Some("Example Article".to_string()),
        },
    ];
    let insights = compute_browse_day_insights("2026-05-25", &visits);
    assert_eq!(insights.top_urls.len(), 1);
    assert_eq!(insights.top_urls[0].title.as_deref(), Some("Example Article"));
}

#[test]
fn local_hour_of_returns_none_for_visit_times_outside_chrono_representable_range() {
    // Exercises the `_ => None` arm in local_hour_of without going
    // through the SQL path — chrono's timestamp_millis_opt returns
    // None for values past its representable range, and the
    // hour-bucket index must simply be skipped rather than
    // panicking. A single visit avoids the session-gap subtraction,
    // which would itself overflow for an extreme millis value (a
    // pathological case that can only arise from a corrupt archive
    // row and is exercised by the load_day_visits SQL filter
    // separately).
    let visits = vec![DayVisitRow {
        id: 0,
        visit_time_ms: i64::MAX,
        transition_type: Some(0),
        url: "https://example.test/".to_string(),
        title: None,
    }];
    let insights = compute_browse_day_insights("2026-05-25", &visits);
    assert_eq!(insights.total_pages, 1);
    // No representable hour means the panel hour-buckets stay zero.
    assert!(insights.hour_buckets.iter().all(|count| *count == 0));
}

// ── DST-edge coverage for resolve_local_midnight ─────────────────
//
// Each test below grabs the crate-global `test_env_lock` before it
// mutates `TZ`, mirroring how `chrome::tests` synchronises any test
// that races against another reader of `std::env`. Chrono's
// `Local` keeps a per-thread cache that only refreshes after a
// 1-second window even when `TZ` changes, so we run the actual
// resolver call from a freshly-spawned thread — its `TZ_INFO`
// thread-local starts empty and reads the env var on first use.
fn with_tz<R>(tz: &str, run: impl FnOnce() -> R + Send + 'static) -> R
where
    R: Send + 'static,
{
    let _guard =
        crate::utils::test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = std::env::var_os("TZ");
    unsafe {
        std::env::set_var("TZ", tz);
    }
    let outcome = std::thread::spawn(run).join().expect("tz-scoped worker panicked");
    crate::utils::restore_test_env_var("TZ", previous.as_deref());
    outcome
}

#[test]
fn local_day_bounds_ms_resolves_atlantic_azores_fall_back_midnight_as_ambiguous() {
    // Atlantic/Azores falls back at 01:00 DST → 00:00 STD, so the
    // wall-clock 00:00 on the fall-back Sunday occurs twice. The
    // resolver must take the `Ambiguous → Ok(earliest)` arm rather
    // than bail.
    let bounds = with_tz(":Atlantic/Azores", || local_day_bounds_ms("2026-10-25"));
    let (start, end) = bounds.expect("ambiguous-midnight day must still produce bounds");
    assert!(end > start, "bounds must enclose at least one millisecond");
}

#[test]
fn local_day_bounds_ms_steps_to_01_00_when_toronto_pre_midnight_spring_forward_skips_midnight() {
    // America/Toronto's first recorded DST start (1919-03-30) ran
    // at 23:30 EST → 00:30 EDT — a one-hour spring forward whose
    // gap starts *before* midnight. Chrono's
    // `find_local_time_type_from_local` treats wall-clock midnight
    // on day X as Single only when it is `<=` transition_start;
    // because this transition begins at 23:30 of day X (strictly
    // before midnight of day X+1), the wall-clock 1919-03-31 00:00
    // lands inside the gap and the resolver gets a genuine
    // `LocalResult::None`. 01:00 exists (the gap ends at 00:30
    // EDT), so the retry succeeds via the inner Single arm.
    //
    // We picked this transition deliberately: every modern DST
    // timezone springs at 02:00 or 03:00 wall-clock, which makes
    // midnight always resolve to Single(STD) and leaves this
    // defensive None-arm uncovered without an ahistorical TZ.
    let bounds = with_tz(":America/Toronto", || local_day_bounds_ms("1919-03-31"));
    let (start, end) = bounds.expect("pre-midnight spring-forward must still produce bounds");
    assert!(end > start);
}
#[test]
fn day_queries_seek_time_indexes() {
    for key in [None, Some([73; 32])] {
        let vault = fresh_vault(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for profile in [None, Some(1)] {
                    let sql = day_visits_sql(profile.is_some(), false, true);
                    let plan = s
                        .query(
                            &format!("EXPLAIN QUERY PLAN {sql}"),
                            params![0, i64::MAX, profile, i64::MAX, 4096],
                            |r| Ok(r.get::<String>(3)?),
                        )?
                        .join("\n");
                    assert!(plan.contains("SEARCH visits"), "{plan}");
                    assert!(!plan.contains("SCAN visits") && !plan.contains("SCAN urls"), "{plan}");
                }
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn a_dense_day_spans_read_pages_without_losing_timestamp_ties() {
    for key in [None, Some([73; 32])] {
        let vault = fresh_vault(key);
        insert_visit(
            &vault,
            1,
            "https://dense.test/",
            Some("Dense"),
            local_ms(9, 0, 0),
            TRANSITION_LINK,
        );
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<4100) INSERT INTO visits(url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT 1,?1,'',1,0 FROM n",[local_ms(9,0,0)])?;Ok(())
        }).unwrap();
        let insights = get_browse_day_insights(
            &vault,
            &BrowseDayInsightsRequest { date: "2026-05-25".into(), profile_id: None },
        )
        .unwrap();
        assert_eq!(insights.total_pages, 4101);
        assert_eq!(insights.session_count, 1);
        assert_eq!(insights.top_urls[0].visits, 4101);
        assert_eq!(insights.hour_buckets[9], 4101);
    }
}
