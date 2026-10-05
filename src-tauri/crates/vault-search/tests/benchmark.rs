//! Responsibilities: measure the production encrypted engine at 2.9M page documents.
//! Not responsible for: asserting timing thresholds on a shared development machine.
//! Failure modes: only warm timings are reported; cache or writer defaults are guessed;
//! progressive steps lose results; generated data exceeds 10 GB or survives the run.
//! Each cold rank/count observation starts a process. Next-page timing follows the
//! first page in that process because cursors own their snapshot. OS cache is not purged.
#![cfg(unix)]

#[path = "support/benchmark.rs"]
mod measurements;
mod support;
use measurements::*;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::{config, key, term};
use vault_search::*;

fn child(name: &str, path: &Path, extra: &[(&str, String)]) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([name, "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env("SEARCH_BENCH_DIR", path);
    for (k, v) in extra {
        command.env(k, v);
    }
    assert!(command.status().unwrap().success(), "benchmark child {name} failed");
}

#[test]
#[ignore = "2.9M encrypted documents; run explicitly in release; all data removed"]
fn encrypted_2_9m_pages() {
    if cfg!(debug_assertions) {
        panic!("benchmark requires --release");
    }
    assert!(rss_mib() > 0.0); // Verify the memory probe before building any data.
    let work = tempfile::tempdir().unwrap();
    println!(
        "BENCH {}",
        json!({"kind":"environment","uptime":uptime(),"pages":PAGES,"data_path":work.path()})
    );
    let writers = std::env::var("SEARCH_BENCH_WRITER_MIB")
        .map(|value| vec![value.parse::<usize>().unwrap()])
        .unwrap_or_else(|_| vec![32, 128, 64]);
    for writer_mib in writers {
        let path = work.path().join(format!("writer-{writer_mib}"));
        std::fs::create_dir(&path).unwrap();
        child("build_child", &path, &[("WRITER_MIB", writer_mib.to_string())]);
        assert!(disk_bytes(&path) < 10_000_000_000);
        if writer_mib == 64 {
            for cache_mib in [16, 64, 128] {
                for case in ["common", "medium", "rare", "domain", "eligibility"] {
                    for mode in ["rank", "count", "progress-rank", "progress-count"] {
                        child(
                            "query_child",
                            &path,
                            &[
                                ("CACHE_MIB", cache_mib.to_string()),
                                ("CASE", case.into()),
                                ("MODE", mode.into()),
                            ],
                        );
                    }
                }
            }
        }
        std::fs::remove_dir_all(&path).unwrap();
        assert!(!path.exists());
    }
    let path = work.path().to_owned();
    work.close().unwrap();
    assert!(!path.exists());
    println!("BENCH {}", json!({"kind":"cleanup","removed":path,"exists":path.exists()}));
}

#[test]
#[ignore = "benchmark subprocess"]
fn build_child() {
    let Some(path) = std::env::var_os("SEARCH_BENCH_DIR") else { return };
    let path = Path::new(&path);
    let writer_mib: usize = std::env::var("WRITER_MIB").unwrap().parse().unwrap();
    let cfg = Config { writer_memory_bytes: writer_mib * 1024 * 1024, ..Config::default() };
    let load = uptime();
    let started = Instant::now();
    let index = SearchIndex::create(path, Some(&key()), cfg).unwrap();
    let mut peak_disk = 0;
    index
        .rebuild(
            (0..PAGES).map(|id| Ok(page(id))),
            PAGES,
            |progress| {
                if progress.documents % 100_352 == 0 || progress.complete {
                    peak_disk = peak_disk.max(disk_bytes(path));
                    assert!(peak_disk < 10_000_000_000);
                    println!(
                        "building writer_mib={writer_mib} documents={} seconds={:.2}",
                        progress.documents,
                        started.elapsed().as_secs_f64()
                    );
                }
            },
            || false,
        )
        .unwrap();
    let seconds = started.elapsed().as_secs_f64();
    drop(index);
    println!(
        "BENCH {}",
        json!({"kind":"build","writer_mib":writer_mib,"cache_mib":Config::default().cache_bytes/1048576,"threads":2,"pages":PAGES,"seconds":seconds,"peak_rss_mib":peak_rss_mib(),"disk_bytes":disk_bytes(path),"peak_disk_bytes":peak_disk,"load_start":load,"load_end":uptime()})
    );
}

fn search(index: &SearchIndex, case: &str) -> SearchSession {
    let word = match case {
        "medium" => "webassembly",
        "rare" => "raresentinel",
        _ => "archive",
    };
    let filters = Filters {
        domain: (case == "domain").then(|| "site42.example".into()),
        eligibility: (case == "eligibility")
            .then(|| Arc::new(|id: u64| id % 50 == 0) as Arc<dyn Fn(u64) -> bool + Send + Sync>),
        ..Filters::default()
    };
    index.search(term(word), filters).unwrap()
}
fn expected(case: &str) -> u64 {
    match case {
        "medium" => (PAGES - 1) / 317 + 1,
        "rare" => 1,
        "domain" => (PAGES - 1 - 42) / 20000 + 1,
        "eligibility" => (PAGES - 1) / 50 + 1,
        _ => PAGES,
    }
}
fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed().as_secs_f64() * 1000.)
}
fn median(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

#[test]
#[ignore = "benchmark subprocess"]
fn query_child() {
    let Some(path) = std::env::var_os("SEARCH_BENCH_DIR") else { return };
    let path = Path::new(&path);
    let cache_mib: usize = std::env::var("CACHE_MIB").unwrap().parse().unwrap();
    let case = std::env::var("CASE").unwrap();
    let mode = std::env::var("MODE").unwrap();
    let load = uptime();
    let cfg = Config { cache_bytes: cache_mib * 1024 * 1024, ..config() };
    let (index, open_ms) = timed(|| SearchIndex::open(path, Some(&key()), cfg).unwrap());
    let (session, session_ms) = timed(|| search(&index, &case));
    if mode.starts_with("progress-") {
        progressive_measurement(&session, &case, &mode, cache_mib, open_ms, session_ms, load);
        return;
    }
    let mut first_samples = vec![];
    let mut next_samples = vec![];
    let mut steps = vec![];
    let (cold_ms, cold_next_ms, digest) = if mode == "rank" {
        let (first, cold) = timed(|| session.page(100, None).unwrap());
        let (next, cold_next) = timed(|| session.page(100, first.next.as_ref()).unwrap());
        for _ in 0..9 {
            let (again, ms) = timed(|| session.page(100, None).unwrap());
            assert_eq!(again.hits, first.hits);
            first_samples.push(ms);
            let (again, ms) = timed(|| session.page(100, first.next.as_ref()).unwrap());
            assert_eq!(again.hits, next.hits);
            next_samples.push(ms);
        }
        let (mut handle, prepare_ms) = timed(|| session.start_page(100, None).unwrap());
        steps.push(prepare_ms);
        loop {
            let (step, ms) = timed(|| handle.step(Duration::from_millis(8)).unwrap());
            steps.push(ms);
            if !step.provisional {
                assert_eq!(step.hits, first.hits);
                break;
            }
        }
        let digest = first.hits.iter().chain(&next.hits).fold(0u64, |h, v| {
            h.wrapping_mul(31).wrapping_add(v.page_id).wrapping_add(v.score.to_bits() as u64)
        });
        (cold, cold_next, digest)
    } else {
        let (count, cold) = timed(|| session.count().unwrap());
        assert_eq!(count, expected(&case));
        for _ in 0..9 {
            let (again, ms) = timed(|| session.count().unwrap());
            assert_eq!(again, count);
            first_samples.push(ms);
        }
        let (mut handle, prepare_ms) = timed(|| session.start_count().unwrap());
        steps.push(prepare_ms);
        loop {
            let (step, ms) = timed(|| handle.step(Duration::from_millis(8)).unwrap());
            steps.push(ms);
            if !step.provisional {
                assert_eq!(step.count, count);
                break;
            }
        }
        (cold, 0., count)
    };
    let cache = index.cache_stats().unwrap();
    assert!(cache.bytes <= cache.limit);
    let step_max = steps.iter().copied().fold(0., f64::max);
    let step_total: f64 = steps.iter().sum();
    let step_count = steps.len() - 1;
    let result = json!({"kind":"query","case":case,"mode":mode,"cache_mib":cache_mib,"matches":expected(&case),"open_ms":open_ms,"session_ms":session_ms,"cold_ms":cold_ms,"cold_next_ms":cold_next_ms,"warm_ms":median(&mut first_samples),"warm_next_ms":if next_samples.is_empty(){0.}else{median(&mut next_samples)},"progress_steps":step_count,"progress_max_ms":step_max,"progress_total_ms":step_total,"progress_step_ms":steps,"steady_rss_mib":rss_mib(),"peak_rss_mib":peak_rss_mib(),"cache_bytes":cache.bytes,"digest":digest,"load_start":load,"load_end":uptime()});
    println!("BENCH {result}");
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Ranked(Vec<Hit>),
    Counted(u64),
}

fn progressive(session: &SearchSession, ranking: bool) -> (Outcome, Vec<f64>) {
    let mut timings = vec![];
    if ranking {
        let (mut handle, prepare) = timed(|| session.start_page(100, None).unwrap());
        timings.push(prepare);
        loop {
            let (step, ms) = timed(|| handle.step(Duration::from_millis(8)).unwrap());
            timings.push(ms);
            if !step.provisional {
                return (Outcome::Ranked(step.hits), timings);
            }
        }
    } else {
        let (mut handle, prepare) = timed(|| session.start_count().unwrap());
        timings.push(prepare);
        loop {
            let (step, ms) = timed(|| handle.step(Duration::from_millis(8)).unwrap());
            timings.push(ms);
            if !step.provisional {
                return (Outcome::Counted(step.count), timings);
            }
        }
    }
}
fn progressive_measurement(
    session: &SearchSession,
    case: &str,
    mode: &str,
    cache_mib: usize,
    open_ms: f64,
    session_ms: f64,
    load: String,
) {
    let ranking = mode == "progress-rank";
    let (cold, cold_steps) = progressive(session, ranking);
    let expected = if ranking {
        Outcome::Ranked(session.page(100, None).unwrap().hits)
    } else {
        Outcome::Counted(expected(case))
    };
    assert_eq!(cold, expected);
    let mut totals = vec![];
    let mut warm_steps = vec![];
    for _ in 0..9 {
        let (outcome, steps) = progressive(session, ranking);
        assert_eq!(outcome, expected);
        totals.push(steps.iter().sum::<f64>());
        warm_steps.push(steps);
    }
    println!(
        "BENCH {}",
        json!({"kind":"progress","case":case,"mode":mode,"cache_mib":cache_mib,"open_ms":open_ms,"session_ms":session_ms,"cold_total_ms":cold_steps.iter().sum::<f64>(),"cold_prepare_ms":cold_steps[0],"cold_steps_ms":&cold_steps[1..],"warm_total_ms":median(&mut totals),"warm_steps_ms":warm_steps,"steady_rss_mib":rss_mib(),"peak_rss_mib":peak_rss_mib(),"load_start":load,"load_end":uptime()})
    );
}
