//! Common paths for a date range, counted from the sessions' stored path segments.
//!
//! Ways this can go wrong, each covered below: a path that crosses from one segment into the
//! next (the size bound, local midnight, a rebuild batch boundary) is counted twice or not at
//! all; an append that continues the open session saves its segment again and counts its paths
//! again; a site visited twice in a row splits into two steps; a date range picks paths by the
//! wrong day.
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture, sync_archive_totals};
use crate::intelligence::intelligence_rebuild::run_core_intelligence;
use crate::intelligence::{get_path_flows, local_date_key, path_flows::is_path};
use crate::models::{CoreIntelligenceRebuildRequest, DateRange, PathFlowRequest};
use crate::test_support::native::TestDatabase;
use chrono::{Local, NaiveDate};
use std::collections::BTreeMap;

const PROFILE: &str = "chrome:Default";
const SITES: usize = 6;

/// Visit times and sites: one session from 23:00 local on 2025-06-10, a visit a minute for 700
/// minutes (past midnight, and more sites in a row than one segment holds), then after a
/// two-hour gap a second session. Sites follow a fixed pseudo-random walk over six sites, so
/// repeats, round trips and real paths all occur.
fn visits(count: usize, first_id: i64, start_ms: i64, seed: u64) -> Vec<(i64, String, i64)> {
    let mut state = seed;
    (0..count)
        .map(|offset| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let site = format!("site{}.dev", (state >> 33) as usize % SITES);
            (first_id + offset as i64, site, start_ms + offset as i64 * 60_000)
        })
        .collect()
}

fn local_ms(date: NaiveDate, hour: u32) -> i64 {
    date.and_hms_opt(hour, 0, 0)
        .and_then(|time| time.and_local_timezone(Local).earliest())
        .expect("local time")
        .timestamp_millis()
}

fn append(archive: &TestDatabase, visits: &[(i64, String, i64)]) {
    let visits = visits.to_vec();
    archive
        .write(move |s| {
            for (id, site, time) in &visits {
                append_fixture_visit(
                    s,
                    *id,
                    &format!("https://{site}/page/{id}"),
                    "Page",
                    *time,
                    None,
                    None,
                );
            }
            Ok(())
        })
        .unwrap();
    sync_archive_totals(archive);
}

/// Every flow of one length the read returns for `range`, all pages.
fn read(vault: &crate::Vault, range: DateRange, steps: u32) -> BTreeMap<String, (i64, String)> {
    let mut flows = BTreeMap::new();
    let mut cursor = None;
    loop {
        let page = get_path_flows(
            vault,
            &PathFlowRequest {
                cursor,
                date_range: range.clone(),
                profile_id: Some(PROFILE.into()),
                step_count: steps,
                limit: Some(1000),
            },
        )
        .unwrap();
        for flow in &page.items {
            flows.insert(
                flow.flow_pattern.clone(),
                (flow.occurrence_count, flow.last_seen_at.clone()),
            );
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return flows,
        }
    }
}

/// The paths of `steps` sites a range should show, counted the slow way: split the visits into
/// sessions at a 30-minute gap, merge visits in a row to the same site, and keep each path
/// whose last site was first visited on a day in the range.
fn expected(
    visits: &[(i64, String, i64)],
    days: &[String],
    steps: usize,
) -> BTreeMap<String, (i64, i64)> {
    let mut sessions: Vec<Vec<(String, i64, i64)>> = Vec::new();
    let mut last_time = None;
    for (_, site, time) in visits {
        if last_time.is_none_or(|last| time - last > 30 * 60_000) {
            sessions.push(Vec::new());
        }
        last_time = Some(*time);
        let runs = sessions.last_mut().unwrap();
        match runs.last_mut() {
            Some((last, _, end)) if last == site => *end = *time,
            _ => runs.push((site.clone(), *time, *time)),
        }
    }
    let mut flows = BTreeMap::new();
    for runs in &sessions {
        for window in runs.windows(steps) {
            let (_, first, last) = &window[steps - 1];
            if !days.contains(&local_date_key(*first)) {
                continue;
            }
            let path: Vec<(String, i64)> =
                window.iter().map(|(site, _, end)| (site.clone(), *end)).collect();
            if !is_path(&path) {
                continue;
            }
            let pattern = path.iter().map(|(site, _)| site.as_str()).collect::<Vec<_>>();
            let entry = flows.entry(pattern.join(" → ")).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(*last);
        }
    }
    flows
}

#[test]
fn a_range_counts_each_path_once_across_segments_batches_and_appends() {
    let vault = crate::Vault::open_temp().unwrap();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    let day = NaiveDate::from_ymd_opt(2025, 6, 10).unwrap();
    let mut all = visits(700, 100, local_ms(day, 23), 7);
    let second_start = all.last().unwrap().2 + 2 * 60 * 60_000;
    all.extend(visits(50, 1_000, second_start, 11));
    append(&archive, &all);
    // Batches of 37 visits: sessions, segments and runs of one site are cut by batch boundaries.
    run_core_intelligence(
        &vault,
        &CoreIntelligenceRebuildRequest { limit: Some(37), ..Default::default() },
    )
    .unwrap();
    // An append that continues the second session, whose last segment is already saved.
    let more = visits(120, 2_000, all.last().unwrap().2 + 60_000, 13);
    append(&archive, &more);
    run_core_intelligence(
        &vault,
        &CoreIntelligenceRebuildRequest { limit: Some(37), ..Default::default() },
    )
    .unwrap();
    all.extend(more);

    let today = Local::now().format("%Y-%m-%d").to_string();
    let everything = DateRange { start: "2000-01-01".into(), end: "2099-12-31".into() };
    for steps in [2, 3, 4] {
        // The stored all-time flows, built by another pass over every visit, are the reference.
        let stored =
            read(&vault, DateRange { start: "1900-01-01".into(), end: today.clone() }, steps);
        assert!(!stored.is_empty(), "no {steps}-site paths in the fixture");
        assert_eq!(read(&vault, everything.clone(), steps), stored, "{steps}-site paths");

        let next_day = local_date_key(local_ms(day.succ_opt().unwrap(), 12));
        let shown =
            read(&vault, DateRange { start: next_day.clone(), end: next_day.clone() }, steps);
        let want = expected(&all, &[next_day], steps as usize);
        assert_eq!(
            shown.iter().map(|(pattern, (count, _))| (pattern.clone(), *count)).collect::<Vec<_>>(),
            want.iter().map(|(pattern, (count, _))| (pattern.clone(), *count)).collect::<Vec<_>>(),
            "{steps}-site paths on the day after midnight"
        );
        for (pattern, (_, last_seen)) in &want {
            assert_eq!(
                shown[pattern].1,
                crate::intelligence::rfc3339_from_millis(*last_seen),
                "last time of {pattern}"
            );
        }
    }
}
