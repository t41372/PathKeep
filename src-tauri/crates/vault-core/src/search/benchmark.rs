//! Responsibilities: measure the encrypted owner path with a complete 14.4M-visit corpus.
//! Not responsible for: substituting engine-only timings or claiming target hardware parity.
//! Failure modes: warm caches called cold, sampled counts, unbounded fixture allocation,
//! disk exceeding 40 GB, missing cleanup, or process high-water called steady RSS.
//! This ignored test uses independent query processes, known counts, and RAII cleanup.
mod backup;
mod fixture;
mod measure;
use super::{SearchRequest, SearchSession};
use serde_json::json;
use std::{process::Command, time::Instant};
const PAGES: i64 = 2_900_000;
const VISITS: i64 = 14_400_000;
const KEY: [u8; 32] = [91; 32];

#[test]
#[ignore = "encrypted 14.4M visits / 2.9M pages; removes its generated data on exit"]
fn encrypted_archive_search() {
    if let Some(root) = std::env::var_os("PATHKEEP_SEARCH_BENCH_ROOT") {
        let root = std::path::Path::new(&root);
        let vault = fixture::open(root);
        let case = std::env::var("PATHKEEP_SEARCH_BENCH_CASE").unwrap();
        if case == "backup" {
            backup::run(&vault);
        } else if case == "rebuild" {
            let started = Instant::now();
            vault.search().rebuild().unwrap();
            println!(
                "{}",
                json!({"case":"rebuild","seconds":started.elapsed().as_secs_f64(),"peakRssMiB":measure::peak(),"steadyRssMiB":measure::rss(),"uptime":measure::uptime()})
            );
        } else {
            let (request, expected) = match case.as_str() {
                "common" => {
                    (SearchRequest { query: "archive".into(), ..Default::default() }, PAGES)
                }
                "medium" => (
                    SearchRequest { query: "webassembly".into(), ..Default::default() },
                    PAGES / 317,
                ),
                "rare" => (SearchRequest { query: "raresentinel".into(), ..Default::default() }, 1),
                "date" => (
                    SearchRequest {
                        query: "archive".into(),
                        start_time_ms: Some(fixture::BASE + (VISITS - VISITS / 50) * 1000 + 1),
                        ..Default::default()
                    },
                    VISITS / 50 / 5,
                ),
                "profile" => (
                    SearchRequest {
                        query: "archive".into(),
                        profile_id: Some("bench:1".into()),
                        ..Default::default()
                    },
                    PAGES / 3,
                ),
                _ => panic!("unknown case"),
            };
            for run in 0..4 {
                let start = Instant::now();
                let mut session = SearchSession::start(&vault, request.clone()).unwrap();
                let mut first = None;
                let mut count_time = None;
                let mut final_page = None;
                let mut max_step = 0f64;
                let mut steps = 0;
                let result = loop {
                    let tick = Instant::now();
                    let step = session.step(std::time::Duration::from_millis(8)).unwrap();
                    max_step = max_step.max(tick.elapsed().as_secs_f64() * 1000.);
                    steps += 1;
                    if first.is_none() && !step.items.is_empty() {
                        first = Some(start.elapsed().as_secs_f64() * 1000.);
                    }
                    if final_page.is_none() && !step.provisional {
                        final_page = Some(start.elapsed().as_secs_f64() * 1000.);
                    }
                    if count_time.is_none() && step.count.exact {
                        count_time = Some(start.elapsed().as_secs_f64() * 1000.);
                    }
                    if !step.provisional && step.count.exact {
                        break step;
                    }
                };
                assert_eq!(result.count.value, expected as u64, "{case}");
                let next_ms = if let Some(cursor) = result.cursor.as_deref() {
                    let next = Instant::now();
                    session.next_page(cursor).unwrap();
                    let next_result = session.finish_page().unwrap();
                    assert!(
                        result
                            .items
                            .iter()
                            .all(|a| next_result.items.iter().all(|b| a.page_id != b.page_id))
                    );
                    Some(next.elapsed().as_secs_f64() * 1000.)
                } else {
                    None
                };
                println!(
                    "{}",
                    json!({"case":case,"run":run,"cache":if run==0{"fresh-process"}else{"warm"},"firstProvisionalMs":first,"firstPageMs":final_page,"nextPageMs":next_ms,"countMs":count_time,"count":result.count.value,"steps":steps,"maxStepMs":max_step,"steadyRssMiB":measure::rss(),"peakRssMiB":measure::peak(),"uptime":measure::uptime()})
                );
            }
        }
        vault.close().unwrap();
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_owned();
    let vault = fixture::open(&path);
    fixture::seed(&vault);
    vault.search().cycle().unwrap();
    println!(
        "{}",
        json!({"case":"seed","bytes":measure::disk(&path),"peakRssMiB":measure::peak(),"uptime":measure::uptime()})
    );
    vault.close().unwrap();
    drop(vault);
    for case in ["common", "medium", "rare", "date", "profile", "rebuild", "backup"] {
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "search::benchmark::encrypted_archive_search",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("PATHKEEP_SEARCH_BENCH_ROOT", &path)
            .env("PATHKEEP_SEARCH_BENCH_CASE", case)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(measure::disk(&path) < 40_000_000_000);
    }
    root.close().unwrap();
    assert!(!path.exists());
    println!("{}", json!({"case":"cleanup","exists":path.exists()}));
}
