//! A real Tantivy index through `EncryptedDirectory`: built, committed, merged, reopened and
//! queried with exact expected answers; no plaintext anywhere on disk at any point; and the
//! cache's memory bound.
//! Failure modes are listed in `src/search_dir/mod.rs`.

mod common;

use std::time::{Duration, Instant};

use common::pages;
use common::{ChildProcess, Watcher, is_child};
use tantivy::directory::MmapDirectory;
use tantivy::store::Compressor;
use tantivy::{Directory, Index, IndexSettings, IndexWriter, TantivyDocument};
use vault_crypto::{DataKey, EncryptedDirectory};
use zeroize::Zeroizing;

const PAGES: u64 = 100_000;
const COMMITS: u64 = 4;

fn data_key() -> DataKey {
    DataKey::from_bytes(Zeroizing::new([5u8; 32]))
}

/// Uncompressed stored fields, so the plaintext control index really shows the text and the
/// scan for it means something.
fn settings() -> IndexSettings {
    IndexSettings { docstore_compression: Compressor::None, ..IndexSettings::default() }
}

/// Builds `PAGES` pages in `COMMITS` commits, then merges everything into one segment.
fn build(directory: Box<dyn Directory>) -> Index {
    let (schema, fields) = pages::schema();
    let index = Index::create(directory, schema, settings()).unwrap();
    let mut writer: IndexWriter = index.writer_with_num_threads(2, 40_000_000).unwrap();
    writer.set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
    let per_commit = PAGES / COMMITS;
    for batch in 0..COMMITS {
        pages::add_pages(&writer, &fields, batch * per_commit..(batch + 1) * per_commit, batch);
        let mut commit = writer.prepare_commit().unwrap();
        commit.set_payload(&format!("pk-payload-sentinel batch {batch}"));
        commit.commit().unwrap();
    }
    let segments = index.searchable_segment_ids().unwrap();
    assert_eq!(
        segments.len() as u64,
        COMMITS * 2,
        "two indexing threads, one segment each per commit"
    );
    writer.merge(&segments).wait().unwrap();
    writer.garbage_collect_files().wait().unwrap();
    writer.wait_merging_threads().unwrap();
    index
}

/// Scores and ids of the top `k`, checked against another index: equal score lists, and the
/// same ids above the k-th score (ids that tie at the k-th score may differ by doc order).
fn assert_same_top(word: &str, encrypted: &[(u64, f32)], reference: &[(u64, f32)]) {
    let scores = |hits: &[(u64, f32)]| hits.iter().map(|h| h.1).collect::<Vec<_>>();
    assert_eq!(scores(encrypted), scores(reference), "{word}: scores differ");
    if let Some(&(_, kth)) = encrypted.last() {
        let above = |hits: &[(u64, f32)]| {
            let mut ids: Vec<u64> = hits.iter().filter(|h| h.1 > kth).map(|h| h.0).collect();
            ids.sort();
            ids
        };
        assert_eq!(above(encrypted), above(reference), "{word}: ids differ");
    }
}

fn sentinel_needles() -> Vec<&'static [u8]> {
    vec![b"pkneedle", b"pksentinel.example", b"pk-payload-sentinel", b"pk_title"]
}

#[test]
fn a_100k_page_index_survives_commit_merge_and_reopen() {
    let work = tempfile::tempdir().unwrap();
    let encrypted_dir = work.path().join("encrypted");
    let plain_dir = work.path().join("plain");
    std::fs::create_dir(&encrypted_dir).unwrap();
    std::fs::create_dir(&plain_dir).unwrap();

    // Build both, each watched so files that merges delete are kept for the scan.
    let started = Instant::now();
    let watcher =
        Watcher::start(vec![encrypted_dir.clone()], work.path().join("quarantine-encrypted"));
    let directory =
        EncryptedDirectory::open(&encrypted_dir, &data_key(), vault_crypto::DEFAULT_CACHE_BYTES)
            .unwrap();
    drop(build(Box::new(directory)));
    let encrypted_files = watcher.finish(&sentinel_needles());
    let build_encrypted = started.elapsed();

    let started = Instant::now();
    let watcher = Watcher::start(vec![plain_dir.clone()], work.path().join("quarantine-plain"));
    let reference = build(Box::new(MmapDirectory::open(&plain_dir).unwrap()));
    let plain_files = watcher.finish(&sentinel_needles());
    let build_plain = started.elapsed();
    println!("built {PAGES} pages: encrypted {build_encrypted:?}, MmapDirectory {build_plain:?}");

    // The control shows the scan works: plaintext files do contain the words.
    let plain_hits: Vec<_> = plain_files.iter().flat_map(|(_, hits)| hits.clone()).collect();
    for needle in sentinel_needles() {
        let needle = String::from_utf8_lossy(needle);
        assert!(plain_hits.iter().any(|h| *h == needle), "control index does not show {needle}");
    }
    let leaks: Vec<_> = encrypted_files.iter().filter(|(_, hits)| !hits.is_empty()).collect();
    assert!(leaks.is_empty(), "plaintext in the encrypted index: {leaks:?}");
    assert!(encrypted_files.len() >= 20, "watcher saw only {} files", encrypted_files.len());
    println!(
        "scanned {} encrypted files ({} bytes at their largest), none with plaintext",
        encrypted_files.len(),
        encrypted_files.iter().map(|(f, _)| f.peak_bytes).sum::<u64>()
    );

    // Reopen from disk with a new directory object and an empty cache.
    let (_, fields) = pages::schema();
    let directory =
        EncryptedDirectory::open(&encrypted_dir, &data_key(), vault_crypto::DEFAULT_CACHE_BYTES)
            .unwrap();
    let index = Index::open(directory.clone()).unwrap();
    let meta = index.load_metas().unwrap();
    assert_eq!(
        meta.payload.as_deref(),
        Some(format!("pk-payload-sentinel batch {}", COMMITS - 1).as_str())
    );
    assert_eq!(meta.segments.len(), 1, "merge left {} segments", meta.segments.len());
    let searcher = index.reader().unwrap().searcher();
    let reference_searcher = reference.reader().unwrap().searcher();
    assert_eq!(searcher.num_docs(), PAGES);

    for (word, expected) in pages::probe_words(0..PAGES) {
        let query = pages::title_term(&fields, &word);
        assert_eq!(pages::count(&searcher, &*query).unwrap(), expected, "count of {word}");
        let top = pages::top(&searcher, &fields, &*query, 100).unwrap();
        let reference_top = pages::top(&reference_searcher, &fields, &*query, 100).unwrap();
        assert_same_top(&word, &top, &reference_top);
        assert_eq!(top.len(), expected.min(100));
        println!("{word:>12}: {expected} pages, top {} identical to MmapDirectory", top.len());
    }

    // Stored fields come back exactly as generated.
    for id in [0, 1, 4242, PAGES / 2, PAGES - 1] {
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_u64(fields.id, id),
            tantivy::schema::IndexRecordOption::Basic,
        );
        let hits = searcher
            .search(&query, &tantivy::collector::TopDocs::with_limit(2).order_by_score())
            .unwrap();
        assert_eq!(hits.len(), 1);
        let doc: TantivyDocument = searcher.doc(hits[0].1).unwrap();
        let page = pages::page(id);
        let text = |field| {
            tantivy::schema::Value::as_str(&doc.get_first(field).unwrap()).unwrap().to_owned()
        };
        assert_eq!(text(fields.title), page.title);
        assert_eq!(text(fields.url), page.url);
    }
    let stats = directory.cache_stats();
    println!("cache after queries: {stats:?}");
    assert!(stats.bytes <= stats.limit);
}

/// The cache limit holds while a search workload reads an index several times its size,
/// measured both by the cache's own count and by the process's resident memory. Each run is
/// a fresh child process.
#[test]
fn the_block_cache_bounds_memory() {
    let work = tempfile::tempdir().unwrap();
    let index_dir = work.path().join("index");
    std::fs::create_dir(&index_dir).unwrap();
    {
        let directory = EncryptedDirectory::open(&index_dir, &data_key(), 1024 * 1024).unwrap();
        let (schema, fields) = pages::schema();
        let index = Index::create(directory, schema, IndexSettings::default()).unwrap();
        let mut writer: IndexWriter = index.writer_with_num_threads(1, 40_000_000).unwrap();
        pages::add_pages(&writer, &fields, 0..PAGES, 0);
        writer.commit().unwrap();
        writer.wait_merging_threads().unwrap();
    }
    let index_bytes: u64 =
        std::fs::read_dir(&index_dir).unwrap().map(|e| e.unwrap().metadata().unwrap().len()).sum();
    let mut results = Vec::new();
    for limit_mib in [2usize, 4096] {
        let child = ChildProcess::spawn(
            "child_cache_workload",
            &[("INDEX_DIR", index_dir.to_str().unwrap()), ("CACHE_MIB", &limit_mib.to_string())],
        );
        let lines = child.wait_success(Duration::from_secs(900));
        let line = lines.iter().find(|l| l.contains("cache-result")).unwrap().clone();
        println!("{line}");
        let field = |name: &str| -> f64 {
            line.split_whitespace()
                .find_map(|w| w.strip_prefix(&format!("{name}=")))
                .unwrap()
                .parse()
                .unwrap()
        };
        results.push((limit_mib, field("max_cache_mib"), field("rss_growth_mib")));
    }
    println!("index on disk: {:.1} MiB", index_bytes as f64 / 1048576.0);
    for &(limit, max_cache, _) in &results {
        assert!(max_cache <= limit as f64, "cache held {max_cache} MiB with a {limit} MiB limit");
    }
    let (small, unbounded) = (results[0].2, results[1].2);
    assert!(
        small + 4.0 < unbounded,
        "a 2 MiB cache grew resident memory by {small:.1} MiB, an unbounded one by {unbounded:.1} MiB"
    );
}

#[test]
#[ignore = "runs only as a child of the_block_cache_bounds_memory"]
fn child_cache_workload() {
    if !is_child() {
        return;
    }
    let index_dir = std::path::PathBuf::from(std::env::var("INDEX_DIR").unwrap());
    let limit: usize = std::env::var("CACHE_MIB").unwrap().parse::<usize>().unwrap() * 1024 * 1024;
    let directory = EncryptedDirectory::open(&index_dir, &data_key(), limit).unwrap();
    let (_, fields) = pages::schema();
    let index = Index::open(directory.clone()).unwrap();
    let reader = index.reader().unwrap();
    let rss_before = common::current_rss_mib(std::process::id());
    let mut max_cache = 0usize;
    let words: Vec<String> = pages::probe_words(0..2_000).into_iter().map(|(w, _)| w).collect();
    for _ in 0..2 {
        let searcher = reader.searcher();
        for word in &words {
            let query = pages::title_term(&fields, word);
            pages::top(&searcher, &fields, &*query, 100).unwrap();
            max_cache = max_cache.max(directory.cache_stats().bytes);
        }
        // Read every stored document: touches every block of the doc store.
        for segment in searcher.segment_readers() {
            let store = segment.get_store_reader(0).unwrap();
            for doc in 0..segment.max_doc() {
                let _: TantivyDocument = store.get(doc).unwrap();
                if doc % 5_000 == 0 {
                    max_cache = max_cache.max(directory.cache_stats().bytes);
                }
            }
        }
        max_cache = max_cache.max(directory.cache_stats().bytes);
    }
    let rss_after = common::current_rss_mib(std::process::id());
    println!(
        "cache-result limit_mib={} max_cache_mib={:.2} rss_before_mib={rss_before:.1} rss_after_mib={rss_after:.1} rss_growth_mib={:.1} hits={} misses={}",
        limit / 1048576,
        max_cache as f64 / 1048576.0,
        rss_after - rss_before,
        directory.cache_stats().hits,
        directory.cache_stats().misses,
    );
}
