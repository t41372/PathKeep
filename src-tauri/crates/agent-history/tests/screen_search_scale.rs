//! Disk-backed index and interactive stream timings. Seed the post-backup projection outside
//! the timed section, so the build figure excludes fixture generation and native discovery.
use agent_history::{Archive, Harness, SourceRoot, capture::BackupOptions, search::ScreenQuery};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

#[path = "support/search_corpus.rs"]
mod corpus;
use corpus::seed;
#[test]
#[ignore = "Linux release conversation search, 120,000 roots x 120 records"]
fn global_stream_at_scale() {
    let roots = std::env::var("AH_SEARCH_ROOTS").map_or(120_000, |n| n.parse().unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("AH_SEARCH_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| tmp.path().join("archive"));
    if !root.exists() {
        seed(&root, roots);
    }
    let a = Arc::new(Archive::open(&root, b"").unwrap());
    let started = Instant::now();
    let built = a.rebuild_search().unwrap();
    assert_eq!(built.indexed_events, (roots * 120) as u64);
    println!(
        "{}",
        json!({"operation":"indexBuild","roots":roots,"records":built.indexed_events,"elapsedMs":started.elapsed().as_millis()})
    );
    for (term, groups, expected) in [("commonword", roots, roots * 30), ("singularneedle", 1, 1)] {
        let start = Instant::now();
        let mut first = None;
        let mut last = None;
        let mut previous = None;
        let mut distinct = 0;
        a.stream_conversations(
            None,
            &ScreenQuery::all(term.into()),
            &Default::default(),
            |ty, value| {
                if ty == "groups" {
                    first.get_or_insert_with(|| start.elapsed());
                    for card in value["items"].as_array().unwrap() {
                        let id = card["id"].as_str().unwrap();
                        if previous.as_deref() != Some(id) {
                            let rank = if groups == 1 { 0 } else { groups - distinct - 1 };
                            assert_eq!(id, format!("pi-{rank:06}"));
                            distinct += 1;
                            previous = Some(id.to_owned());
                        }
                        if groups == 1 {
                            assert_eq!(card["snippets"][0]["address"]["ordinal"], 119);
                        }
                    }
                }
                if ty == "finished" {
                    last = Some(value)
                }
                Ok(())
            },
        )
        .unwrap();
        let last = last.unwrap();
        assert_eq!(distinct, groups);
        assert_eq!(last["hits"], expected);
        assert_eq!(last["searchedCount"], roots);
        println!(
            "{}",
            json!({"operation":"search","query":term,"firstResultMs":first.unwrap().as_secs_f64()*1000.,"endMs":start.elapsed().as_secs_f64()*1000.,"hits":expected,"conversations":groups,"searched":roots})
        );
    }
    let path = tmp.path().join("pi");
    std::fs::create_dir_all(path.join("sessions")).unwrap();
    std::fs::write(path.join("sessions/new.jsonl"),b"{\"type\":\"session\",\"id\":\"added\",\"version\":3}\n{\"type\":\"message\",\"id\":\"u\",\"parentId\":null,\"message\":{\"role\":\"user\",\"content\":\"a new question\"}}\n{\"type\":\"message\",\"id\":\"a\",\"parentId\":\"u\",\"message\":{\"role\":\"assistant\",\"content\":\"deltaonly\"}}\n").unwrap();
    let elapsed = Arc::new(Mutex::new(None));
    let observed = elapsed.clone();
    let owner = a.clone();
    let start = Instant::now();
    a.backup(
        &[SourceRoot { harness: Harness::Pi, root: path }],
        &BackupOptions {
            progress: Some(Arc::new(move |p| {
                if p.phase == "index" {
                    let start = Instant::now();
                    let delta = owner.synchronize_search().unwrap();
                    *observed.lock().unwrap() = Some((start.elapsed(), delta.indexed_events));
                }
            })),
            ..Default::default()
        },
    )
    .unwrap();
    let (index_time, events) = elapsed.lock().unwrap().unwrap();
    let backup = start.elapsed();
    let mut hits = None;
    a.stream_conversations(
        None,
        &ScreenQuery::all("deltaonly".into()),
        &Default::default(),
        |ty, p| {
            if ty == "finished" {
                hits = p["hits"].as_i64()
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(hits, Some(1));
    println!(
        "{}",
        json!({"operation":"oneNewRoot","indexedEvents":events,"indexUpdateMs":index_time.as_secs_f64()*1000.,"backupMs":backup.as_secs_f64()*1000.})
    );
}
