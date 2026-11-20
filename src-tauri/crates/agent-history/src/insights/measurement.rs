//! Exercise the production first-paint and range-change entry point with published facts on
//! disk. Native ingestion has separate fixture oracles. Seeding is outside the timed reads.
use super::{
    clock,
    facts::{Brief, DurationKind, Fact},
    *,
};
use crate::{Archive, read_model::usage::Tokens};
use chrono::{Days, NaiveDate, TimeZone};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

const ZONE: &str = "America/New_York";
fn end_day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
}
fn seed(path: &Path, days: u64) {
    drop(Archive::create_unencrypted(path).unwrap());
    let mut db = rusqlite::Connection::open(vault_layout::path(
        path,
        vault_layout::ArtifactId::AgentHistoryReadModel,
    ))
    .unwrap();
    let zone: chrono_tz::Tz = ZONE.parse().unwrap();
    for day in 0..days {
        let tx = db.transaction().unwrap();
        {
            let mut root=tx.prepare("INSERT INTO ah_chat_summary(publication,id,live,root,harness,folder,title,title_source,started,last_activity,missing,bytes,detail,own,totals,recent_key,start_key,agent_key,tokens_key,turns_key,turns,agent_ms,tokens_total,insight_first,insight_last,insight_first_usage)
              VALUES(?1,?1,1,1,?2,?3,?1,'tool',?4,?5,0,48000,'{}','{}','{}',?1,?1,?1,?1,?1,30,720000,?6,?4,?5,CASE WHEN ?6 IS NOT NULL THEN ?4+12000 END)").unwrap();
            let mut clock_page =
                tx.prepare("INSERT INTO ah_insight_clock_input VALUES(?1,?1,0,?2,?3)").unwrap();
            let mut page =
                tx.prepare("INSERT INTO ah_insight_page(publication,source,page,lo,hi,undated,brief,detail) VALUES(?1,?1,0,?2,?3,0,?4,?5)").unwrap();
            let date = end_day() - Days::new(days - day - 1);
            for n in 0..300 {
                let id = format!("root-{day:04}-{n:03}");
                let start = zone
                    .from_local_datetime(&date.and_hms_opt(8 + n % 10, n / 10, 0).unwrap())
                    .single()
                    .unwrap()
                    .timestamp_millis();
                let end = start + 29 * 30000 + 24000;
                let measured = n % 5 != 0;
                root.execute(rusqlite::params![
                    id,
                    if n % 2 == 0 { "pi" } else { "claude-code" },
                    format!("/work/p{}", n % 100),
                    start,
                    end,
                    measured.then_some(720)
                ])
                .unwrap();
                let mut facts = Vec::with_capacity(150);
                for turn in 0..30 {
                    let at = start + turn * 30000;
                    facts.push(Fact::Work {
                        at: Some(at),
                        end: Some(at + 24000),
                        source: if n % 2 == 0 {
                            DurationKind::Derived
                        } else {
                            DurationKind::Recorded
                        },
                        turn: true,
                        elapsed: Some(24000),
                    });
                    facts.push(Fact::Prompt { at: Some(at), end: Some(at + 24000) });
                    facts.push(Fact::Call { at: Some(at + 1000), name: "Read".into() });
                    for offset in [12000, 24000] {
                        facts.push(Fact::Reply {
                            at: Some(at + offset),
                            model: "claude-sonnet-4-6".into(),
                            tokens: measured.then_some(Tokens {
                                input: 10,
                                output: 2,
                                ..Default::default()
                            }),
                            recorded: None,
                            estimated: measured.then_some(0.000060),
                        });
                    }
                }
                for (h, input) in super::work::split(&facts) {
                    clock_page
                        .execute(rusqlite::params![id, h, postcard::to_allocvec(&input).unwrap()])
                        .unwrap();
                }
                let mut brief = Brief::default();
                for f in &facts {
                    brief.observe(f);
                }
                brief.spans = clock::union(brief.spans);
                brief.presence = clock::merge_clusters(brief.presence);
                page.execute(rusqlite::params![
                    id,
                    start,
                    end,
                    postcard::to_allocvec(&brief).unwrap(),
                    postcard::to_allocvec(&facts).unwrap()
                ])
                .unwrap();
            }
        }
        tx.commit().unwrap();
    }
    db.execute_batch("INSERT INTO ah_project_paths SELECT DISTINCT folder FROM ah_chat_summary; INSERT INTO ah_project_dirty SELECT path FROM ah_project_paths; INSERT INTO ah_insight_clock_dirty SELECT DISTINCT hour FROM ah_insight_clock_input").unwrap();
    drop(db);
    let a = Archive::open(path, b"").unwrap();
    a.project_list().unwrap();
    let start = Instant::now();
    a.refresh_insight_hours(&Default::default()).unwrap();
    println!(
        "{}",
        json!({"operation":"materializeClock","roots":300*days,"ms":start.elapsed().as_millis()})
    );
}
fn run(a: &Archive, range: Range, part: Part, days: u64) -> Value {
    let now = chrono_tz::America::New_York
        .from_local_datetime(&end_day().and_hms_opt(23, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp_millis();
    let mut overview = None;
    let mut first = None;
    let mut progress = None;
    let mut overview_time = None;
    let mut calendar_time = None;
    let mut work_total = None;
    let start = Instant::now();
    let prefs = a.insight_preferences().unwrap();
    let preferences_ms = start.elapsed().as_secs_f64() * 1000.;
    a.agent_insights(
        &Query { range: Some(range), part, ..Default::default() },
        ZONE,
        now,
        &prefs,
        &Default::default(),
        |ty, p| {
            if ty == "progress" {
                progress.get_or_insert(start.elapsed());
            }
            if !matches!(ty, "started" | "progress") {
                first.get_or_insert(start.elapsed());
            }
            if ty == "work" {
                work_total = Some(
                    p["buckets"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|b| b["busyMs"].as_i64())
                        .sum::<i64>(),
                );
            }
            if ty == "calendar" {
                calendar_time = Some(start.elapsed());
            }
            if ty == "overview" {
                overview_time = Some(start.elapsed());
                overview = Some(p);
            }
            Ok(())
        },
    )
    .unwrap();
    let end = start.elapsed();
    let k = overview.unwrap();
    let d = if range == Range::All { days } else { days.min(30) };
    assert_eq!(k["conversationCount"], 300 * d);
    assert_eq!(k["conversationsWithUsage"], 240 * d);
    assert_eq!(k["tokens"]["output"], 240 * d * 120);
    assert_eq!(k["agentSummedMs"], 300 * d * 720000);
    // Each of ten clock hours covers 88 consecutive half-minute slots (24 s per slot).
    assert_eq!(k["agentBusyMs"], 10 * d * 88 * 24000);
    if let Some(n) = work_total {
        assert_eq!(k["agentBusyMs"], n);
    }
    json!({"preferencesMs":preferences_ms,"overviewMs":overview_time.unwrap().as_secs_f64()*1000.,"calendarMs":calendar_time.map(|t|t.as_secs_f64()*1000.),"firstSectionMs":first.unwrap().as_secs_f64()*1000.,"endMs":end.as_secs_f64()*1000.,"firstProgressMs":progress.map(|p|p.as_secs_f64()*1000.),"roots":k["conversationCount"]})
}
#[test]
#[ignore = "Linux release first paint and range changes, 120000 roots"]
fn insights_120000_roots() {
    let days = std::env::var("AH_INSIGHT_DAYS").ok().map(|s| s.parse().unwrap()).unwrap_or(400);
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("archive");
    seed(&path, days);
    for (first, next) in [(Range::Month, Range::All), (Range::All, Range::Month)] {
        let archive = Archive::open(&path, b"").unwrap();
        let paint = run(&archive, first, Part::Both, days);
        let change = run(&archive, next, Part::Report, days);
        let mut warm = Vec::new();
        for _ in 0..30 {
            warm.push(run(&archive, first, Part::Both, days)["endMs"].as_f64().unwrap());
        }
        warm.sort_by(f64::total_cmp);
        println!(
            "{}",
            json!({"roots":300*days,"turns":9000*days,"replies":18000*days,"calls":9000*days,"zone":ZONE,"initialRange":first,"firstPaint":paint,"changedRange":next,"rangeChange":change,"warmP95Ms":warm[28]})
        );
    }
}
