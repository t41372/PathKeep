//! SIGKILL during indexing and merging leaves an encrypted index that opens at its last
//! commit, answers queries exactly, holds no plaintext, and accepts a new writer.
//!
//! Each run copies a prepared index, starts a child process that commits batches (or merges
//! all segments), and kills it with SIGKILL: at a random moment, or at a named step of the
//! `meta.json` replacement. The child prints `committed <batch>` after each commit returns, so
//! the parent knows the last acknowledged commit. After the kill the parent checks:
//! - the index opens, and its commit payload names the acknowledged batch or the one after
//!   it (killed after the commit landed but before the child printed);
//! - the document count and every batch's count are exactly those of that commit;
//! - no file holds plaintext from the documents, the schema or the payload;
//! - a new writer can take the lock, commit, and garbage-collect what the crash left.
//!
//! Slow; run explicitly, in release:
//! `cargo test --release -p vault-crypto --test search_crash -- --ignored --nocapture`.
//! Set `VAULT_CRYPTO_SEED` to replay a run.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::pages::{self, Fields};
use common::{ChildProcess, Rng, is_child};
use tantivy::collector::Count;
use tantivy::query::TermQuery;
use tantivy::schema::IndexRecordOption;
use tantivy::{Directory, Index, IndexSettings, IndexWriter, Term};
use vault_crypto::{DataKey, EncryptedDirectory};
use zeroize::Zeroizing;

const BATCH: u64 = 5_000;
const BASE_BATCHES: u64 = 4;
const MERGE_SEGMENTS: u64 = 8;
const RUNS: usize = 24;

fn data_key() -> DataKey {
    DataKey::from_bytes(Zeroizing::new([3u8; 32]))
}

fn open_directory(dir: &Path) -> EncryptedDirectory {
    EncryptedDirectory::open(dir, &data_key(), 16 * 1024 * 1024).unwrap()
}

fn needles() -> Vec<&'static [u8]> {
    vec![b"pkneedle", b"pksentinel.example", b"pk_title", b"pk-batch-payload"]
}

/// An index with `batches` committed batches of `BATCH` pages, one segment per batch.
fn prepare(dir: &Path, batches: u64) {
    std::fs::create_dir_all(dir).unwrap();
    let (schema, fields) = pages::schema();
    let index = Index::create(open_directory(dir), schema, IndexSettings::default()).unwrap();
    let mut writer: IndexWriter = index.writer_with_num_threads(1, 40_000_000).unwrap();
    writer.set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
    for batch in 0..batches {
        commit_batch(&mut writer, &fields, batch);
    }
    writer.wait_merging_threads().unwrap();
}

fn commit_batch(writer: &mut IndexWriter, fields: &Fields, batch: u64) {
    pages::add_pages(writer, fields, batch * BATCH..(batch + 1) * BATCH, batch);
    let mut commit = writer.prepare_commit().unwrap();
    commit.set_payload(&format!("pk-batch-payload {batch}"));
    commit.commit().unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

/// Checks the index after a kill and returns (committed batches, segments).
fn check_recovered(dir: &Path, acknowledged: Option<u64>, what: &str) -> (u64, usize) {
    let leaks = common::find_plaintext(dir, &needles());
    assert!(leaks.is_empty(), "{what}: plaintext on disk: {leaks:?}");

    let directory = open_directory(dir);
    let index = Index::open(directory.clone())
        .unwrap_or_else(|e| panic!("{what}: index does not open: {e}"));
    let meta = index.load_metas().unwrap();
    let payload = meta.payload.clone().unwrap_or_else(|| panic!("{what}: no payload"));
    let last: u64 = payload.strip_prefix("pk-batch-payload ").unwrap().parse().unwrap();
    if let Some(acknowledged) = acknowledged {
        assert!(
            last == acknowledged || last == acknowledged + 1,
            "{what}: index is at batch {last}, child acknowledged {acknowledged}"
        );
    }
    let (_, fields) = pages::schema();
    let searcher = index.reader().unwrap().searcher();
    let committed = last + 1;
    assert_eq!(searcher.num_docs(), committed * BATCH, "{what}: document count");
    for batch in 0..committed + 2 {
        let query =
            TermQuery::new(Term::from_field_u64(fields.batch, batch), IndexRecordOption::Basic);
        let expected = if batch < committed { BATCH } else { 0 };
        assert_eq!(
            searcher.search(&query, &Count).unwrap() as u64,
            expected,
            "{what}: batch {batch}"
        );
    }
    // A word query, checked against the generator.
    let query = pages::title_term(&fields, "pkneedle");
    assert_eq!(
        pages::count(&searcher, &*query).unwrap(),
        pages::expected_count("pkneedle", 0..committed * BATCH)
    );
    let segments = meta.segments.len();

    // A new writer gets the lock (the dead child's lock died with it), commits, and cleans up.
    let mut writer: IndexWriter = index.writer_with_num_threads(1, 20_000_000).unwrap();
    writer.set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
    commit_batch(&mut writer, &fields, committed);
    writer.garbage_collect_files().wait().unwrap();
    writer.wait_merging_threads().unwrap();
    let reader = index.reader().unwrap();
    assert_eq!(
        reader.searcher().num_docs(),
        (committed + 1) * BATCH,
        "{what}: after the next commit"
    );
    assert!(directory.integrity_error().is_none(), "{what}: {:?}", directory.integrity_error());
    // Nothing the crash left survives garbage collection: what is on disk is exactly what
    // Tantivy manages, plus its two lock files.
    let managed = json_string_list(&directory.atomic_read(Path::new(".managed.json")).unwrap());
    let mut expected: Vec<String> =
        managed.into_iter().chain(["meta.json".into(), ".managed.json".into()]).collect();
    expected.sort();
    expected.dedup();
    let mut on_disk: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| !n.ends_with(".lock"))
        .collect();
    on_disk.sort();
    assert_eq!(on_disk, expected, "{what}: files left after garbage collection");
    (committed, segments)
}

#[test]
#[ignore = "kills 42 child processes; run explicitly in release"]
fn kills_during_commits_leave_the_last_commit() {
    let work = tempfile::tempdir().unwrap();
    let base = work.path().join("base");
    prepare(&base, BASE_BATCHES);
    let (mut rng, seed) = Rng::from_env_or_clock();
    println!("seed {seed}, load {}", common::load_averages());

    // Calibrate: how long one batch takes, so kills spread over several commits.
    let dir = work.path().join("calibrate");
    copy_dir(&base, &dir);
    let mut child =
        ChildProcess::spawn("child_commit_forever", &[("INDEX_DIR", dir.to_str().unwrap())]);
    child.wait_for_line("ready", Duration::from_secs(120));
    let started = Instant::now();
    child.wait_for_line(&format!("committed {}", BASE_BATCHES + 2), Duration::from_secs(300));
    let per_batch = started.elapsed() / 3;
    child.kill();
    println!("one batch of {BATCH} pages takes {per_batch:?}");

    let mut outcomes = Vec::new();
    for run in 0..RUNS {
        let dir = work.path().join(format!("commit-{run}"));
        copy_dir(&base, &dir);
        let mut child =
            ChildProcess::spawn("child_commit_forever", &[("INDEX_DIR", dir.to_str().unwrap())]);
        child.wait_for_line("ready", Duration::from_secs(120));
        let delay = Duration::from_micros(rng.below(per_batch.as_micros() as u64 * 4));
        std::thread::sleep(delay);
        child.kill();
        let acknowledged = acknowledged(&mut child).unwrap_or(BASE_BATCHES - 1);
        let (committed, _) = check_recovered(
            &dir,
            Some(acknowledged),
            &format!("commit run {run} (killed after {delay:?})"),
        );
        outcomes
            .push(format!("{delay:.0?}: acknowledged {acknowledged}, recovered {}", committed - 1));
        std::fs::remove_dir_all(&dir).unwrap();
    }
    println!("{RUNS} random kills during commits:\n  {}", outcomes.join("\n  "));

    // Kills at each step of replacing meta.json (the commit itself) and .managed.json (the
    // list of files Tantivy owns, rewritten as each new segment file is created).
    let mut steps = 0;
    for file in ["meta.json", ".managed.json"] {
        for step in
            ["atomic-replace/temp-written", "atomic-replace/temp-synced", "atomic-replace/renamed"]
        {
            for nth in [1, 2, 7] {
                let dir = work.path().join("step");
                copy_dir(&base, &dir);
                let mut child = ChildProcess::spawn(
                    "child_commit_forever",
                    &[
                        ("INDEX_DIR", dir.to_str().unwrap()),
                        ("VAULT_CRYPTO_CRASH_AT", &format!("{step}@{file}:{nth}")),
                    ],
                );
                child.wait_for_line(&format!("crash-point {step}"), Duration::from_secs(300));
                child.kill();
                let acknowledged = acknowledged(&mut child).unwrap_or(BASE_BATCHES - 1);
                let (committed, _) =
                    check_recovered(&dir, Some(acknowledged), &format!("{step}@{file}:{nth}"));
                println!(
                    "killed at {step} of {file}, hit {nth}: acknowledged {acknowledged}, recovered {}",
                    committed - 1
                );
                std::fs::remove_dir_all(&dir).unwrap();
                steps += 1;
            }
        }
    }
    println!("{steps} kills at named steps");
}

#[test]
#[ignore = "kills 24 child processes; run explicitly in release"]
fn kills_during_merges_leave_the_last_commit() {
    let work = tempfile::tempdir().unwrap();
    let base = work.path().join("base");
    prepare(&base, MERGE_SEGMENTS);
    let (mut rng, seed) = Rng::from_env_or_clock();
    println!("seed {seed}, load {}", common::load_averages());

    // Calibrate the merge's duration.
    let dir = work.path().join("calibrate");
    copy_dir(&base, &dir);
    let mut child = ChildProcess::spawn("child_merge_all", &[("INDEX_DIR", dir.to_str().unwrap())]);
    child.wait_for_line("ready", Duration::from_secs(120));
    let started = Instant::now();
    child.wait_for_line("merged", Duration::from_secs(600));
    let merge_time = started.elapsed();
    child.wait_success(Duration::from_secs(60));
    assert_eq!(check_recovered(&dir, None, "uninterrupted merge").1, 1);
    println!("merging {MERGE_SEGMENTS} segments of {BATCH} pages takes {merge_time:?}");

    let (mut before, mut after) = (0, 0);
    let mut outcomes = Vec::new();
    for run in 0..RUNS {
        let dir = work.path().join(format!("merge-{run}"));
        copy_dir(&base, &dir);
        let mut child =
            ChildProcess::spawn("child_merge_all", &[("INDEX_DIR", dir.to_str().unwrap())]);
        child.wait_for_line("ready", Duration::from_secs(120));
        let delay = Duration::from_micros(rng.below(merge_time.as_micros() as u64 * 6 / 5));
        std::thread::sleep(delay);
        child.kill();
        let what = format!("merge run {run} (killed after {delay:?})");
        let (committed, segments) = check_recovered(&dir, Some(MERGE_SEGMENTS - 1), &what);
        assert_eq!(committed, MERGE_SEGMENTS, "{what}");
        assert!(
            segments == 1 || segments == MERGE_SEGMENTS as usize,
            "{what}: {segments} segments"
        );
        if segments == 1 {
            after += 1;
        } else {
            before += 1;
        }
        outcomes.push(format!("{delay:.0?}: {}", if segments == 1 { "after" } else { "before" }));
        std::fs::remove_dir_all(&dir).unwrap();
    }
    println!(
        "{RUNS} random kills during a merge: {before} reopened before the merge, {after} after it"
    );
    println!("  {}", outcomes.join(", "));
    assert!(before > 0, "no kill landed before the merge finished");
}

/// `.managed.json` is a JSON array of file names; Tantivy's names need no escaping.
fn json_string_list(bytes: &[u8]) -> Vec<String> {
    let text = std::str::from_utf8(bytes).unwrap().trim();
    let inner = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')).unwrap();
    inner
        .split(',')
        .map(|n| n.trim().trim_matches('"').to_owned())
        .filter(|n| !n.is_empty())
        .collect()
}

fn acknowledged(child: &mut ChildProcess) -> Option<u64> {
    child
        .lines_so_far()
        .iter()
        .filter_map(|line| line.split("committed ").nth(1))
        .filter_map(|n| n.trim().parse().ok())
        .max()
}

/// Child: commits one batch after another until killed.
#[test]
#[ignore = "runs only as a child of the crash tests"]
fn child_commit_forever() {
    if !is_child() {
        return;
    }
    let dir = PathBuf::from(std::env::var("INDEX_DIR").unwrap());
    let index = Index::open(open_directory(&dir)).unwrap();
    let (_, fields) = pages::schema();
    let mut writer: IndexWriter = index.writer_with_num_threads(2, 40_000_000).unwrap();
    let mut batch = BASE_BATCHES;
    println!("ready");
    loop {
        commit_batch(&mut writer, &fields, batch);
        println!("committed {batch}");
        batch += 1;
    }
}

/// Child: merges every segment into one, then exits.
#[test]
#[ignore = "runs only as a child of the crash tests"]
fn child_merge_all() {
    if !is_child() {
        return;
    }
    let dir = PathBuf::from(std::env::var("INDEX_DIR").unwrap());
    let index = Index::open(open_directory(&dir)).unwrap();
    let mut writer: IndexWriter = index.writer_with_num_threads(1, 20_000_000).unwrap();
    writer.set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
    let segments = index.searchable_segment_ids().unwrap();
    println!("ready");
    writer.merge(&segments).wait().unwrap();
    println!("merged");
    writer.garbage_collect_files().wait().unwrap();
    writer.wait_merging_threads().unwrap();
}
