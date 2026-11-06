//! Responsibilities: measure what a typo walk does around it: how soon it stops when the next
//! keystroke replaces it, and how long writes wait while it runs.
//! Not responsible for: timing the walk itself (`benchmark.rs`) or claiming target hardware parity.
//! Failure modes: a session left running between trials, or a trial reported as measuring a walk
//! that had already ended or failed.
//! Used by the ignored encrypted owner benchmark.
use super::{SearchRequest, measure};
use crate::{Vault, search::SearchEvent};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use vault_store::{Db, Priority};

const NO_MATCH: &str = "quartz lantern";
/// The exact pass over these words ends at once, so this is well inside the walk.
const INTO_THE_WALK: Duration = Duration::from_secs(2);

/// Starts a search and returns its session, with a flag that goes up when it has ended: a trial
/// reports it, so a walk that was already over is not read as a walk measured.
fn start(vault: &Vault, query: &str) -> (String, Arc<AtomicBool>) {
    let request = SearchRequest { query: query.into(), ..Default::default() };
    let ended = Arc::new(AtomicBool::default());
    let seen = ended.clone();
    let sink = move |event: SearchEvent| {
        let last = match &event {
            SearchEvent::Step(step) => !step.provisional && step.count.exact,
            SearchEvent::Error(_) | SearchEvent::Cancelled => true,
            SearchEvent::Indexer(_) => false,
        };
        seen.fetch_or(last, Ordering::Release);
        true
    };
    (vault.search_sessions().start(vault, request, sink).unwrap().session_id, ended)
}

/// Each keystroke starts a search and cancels the one before it, the way the History page does.
pub(super) fn cancel(vault: &Vault) {
    let typed =
        ["quartz la", "quartz lan", "quartz lant", "quartz lante", "quartz lanter", NO_MATCH];
    for (run, query) in typed.into_iter().enumerate() {
        let (session, ended) = start(vault, query);
        std::thread::sleep(INTO_THE_WALK);
        let walking = !ended.load(Ordering::Acquire);
        let cancelling = Instant::now();
        vault.search_sessions().cancel(&session).unwrap();
        println!(
            "{}",
            json!({"case":"walk-cancel","run":run,"walking":walking,"cancelMs":cancelling.elapsed().as_secs_f64()*1000.,"cpuSeconds":measure::cpu_seconds(),"steadyRssMiB":measure::rss(),"peakRssMiB":measure::peak(),"uptime":measure::uptime()})
        );
    }
}

/// One small write every 100 ms, the size of saving a note, in milliseconds, sorted.
fn write_waits(vault: &Vault) -> Vec<f64> {
    let mut waits = (0..100)
        .map(|_| {
            std::thread::sleep(Duration::from_millis(100));
            let writing = Instant::now();
            vault
                .store()
                .write(Db::Archive, Priority::Interactive, |s| {
                    s.execute(
                        "UPDATE source_profiles SET profile_name=profile_name WHERE id=1",
                        (),
                    )?;
                    Ok(())
                })
                .unwrap();
            writing.elapsed().as_secs_f64() * 1000.
        })
        .collect::<Vec<_>>();
    waits.sort_by(f64::total_cmp);
    waits
}

pub(super) fn writes(vault: &Vault) {
    let idle = write_waits(vault);
    let (session, ended) = start(vault, NO_MATCH);
    std::thread::sleep(INTO_THE_WALK);
    let walking = write_waits(vault);
    let lasted = !ended.load(Ordering::Acquire);
    vault.search_sessions().cancel(&session).unwrap();
    println!(
        "{}",
        json!({"case":"walk-writes","walkOutlastedTheWrites":lasted,"idleP50Ms":idle[50],"idleP95Ms":idle[95],"idleMaxMs":idle[99],"walkingP50Ms":walking[50],"walkingP95Ms":walking[95],"walkingMaxMs":walking[99],"peakRssMiB":measure::peak(),"uptime":measure::uptime()})
    );
}
