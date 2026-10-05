//! Benchmark: the encrypted directory against Tantivy's `MmapDirectory` at 1M page-shaped
//! documents. Run explicitly, in release:
//! `cargo test --release -p vault-crypto --test search_bench -- --ignored --nocapture`.
//!
//! Both indexes are built from the same pages with the same settings (Tantivy's defaults,
//! LZ4 document store), two indexing threads, then merged into one segment. Every query
//! measurement runs in a fresh child process:
//! - cold: the first query in that process (empty block cache, nothing mapped yet). The OS
//!   file cache is not purged, so this is a cold process, not a cold disk;
//! - warm: the median of 20 more runs of the same query;
//! - 4 threads: four threads running the query 20 times each at once (the target machine has
//!   4 cores), which is where contention on the block cache's lock would show;
//! - peak RSS of the process, and the block cache's counters.
//!
//! A query is a title term, top 100, plus the page id of each hit read from its fast field,
//! which is what the search crate needs before it asks Turso for the rows. Two rankings:
//! - `score`: BM25 only, which lets Tantivy skip blocks of postings (Block-WAND);
//! - `score+recency`: BM25, then the page's last visit from a fast field, PathKeep's planned
//!   order. Ties must be kept, so every match is scored and its fast field read.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::pages;
use common::{ChildProcess, is_child};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::{
    Directory, DocAddress, DocId, Index, IndexSettings, IndexWriter, Score, Searcher, SegmentReader,
};
use vault_crypto::{DataKey, EncryptedDirectory};
use zeroize::Zeroizing;

const PAGES: u64 = 1_000_000;
const WARM_RUNS: usize = 20;

fn data_key() -> DataKey {
    DataKey::from_bytes(Zeroizing::new([0x42; 32]))
}

fn open_directory(variant: &str, dir: &Path, cache_mib: usize) -> Box<dyn Directory> {
    match variant {
        "encrypted" => {
            Box::new(EncryptedDirectory::open(dir, &data_key(), cache_mib * 1024 * 1024).unwrap())
        }
        "mmap" => Box::new(MmapDirectory::open(dir).unwrap()),
        _ => unreachable!(),
    }
}

fn field<'a>(line: &'a str, name: &str) -> &'a str {
    line.split_whitespace().find_map(|w| w.strip_prefix(&format!("{name}="))).unwrap_or("?")
}

#[test]
#[ignore = "benchmark: builds two 1M-document indexes; run explicitly in release"]
fn encrypted_directory_against_mmap_directory() {
    if cfg!(debug_assertions) {
        panic!("run the benchmark in release: debug timings say nothing about the product");
    }
    let work = tempfile::tempdir().unwrap();
    println!("free disk before: {}", free_disk(work.path()));

    // The words: a common one, the most common one (the worst case), and a rare one.
    let mut counts = std::collections::HashMap::<String, u64>::new();
    for id in 0..PAGES {
        let mut words = pages::page(id).words;
        words.sort();
        words.dedup();
        for word in words {
            *counts.entry(word).or_default() += 1;
        }
    }
    let rare =
        counts.iter().filter(|(_, c)| (2..=5).contains(*c)).map(|(w, _)| w.clone()).min().unwrap();
    let words = ["review".to_owned(), "the".to_owned(), rare];
    for word in &words {
        println!("{word}: in {} of {PAGES} pages", counts[word]);
    }

    let mut sizes = Vec::new();
    for variant in ["encrypted", "mmap"] {
        let dir = work.path().join(variant);
        std::fs::create_dir(&dir).unwrap();
        let child = ChildProcess::spawn(
            "child_build",
            &[("INDEX_DIR", dir.to_str().unwrap()), ("VARIANT", variant)],
        );
        let lines = child.wait_success(Duration::from_secs(3600));
        let line = lines.iter().find(|l| l.contains("build-result")).unwrap();
        println!("{}", &line[line.find("build-result").unwrap()..]);
        sizes.push(dir_size(&dir));
    }
    println!(
        "on disk: encrypted {:.1} MiB, MmapDirectory {:.1} MiB (+{:.3}%); free disk now: {}",
        sizes[0] as f64 / 1048576.0,
        sizes[1] as f64 / 1048576.0,
        (sizes[0] as f64 / sizes[1] as f64 - 1.0) * 100.0,
        free_disk(work.path()),
    );

    let queries = [
        (words[0].clone(), "score"),
        (words[1].clone(), "score"),
        (words[2].clone(), "score"),
        (words[0].clone(), "score+recency"),
        (words[1].clone(), "score+recency"),
    ];
    let mut table = Vec::new();
    for (word, ranking) in &queries {
        let mut digests = Vec::new();
        for (variant, cache_mib) in [("encrypted", 64), ("mmap", 0)] {
            let dir = work.path().join(variant);
            let child = ChildProcess::spawn(
                "child_query",
                &[
                    ("INDEX_DIR", dir.to_str().unwrap()),
                    ("VARIANT", variant),
                    ("WORD", word),
                    ("RANKING", ranking),
                    ("CACHE_MIB", &cache_mib.to_string()),
                ],
            );
            let lines = child.wait_success(Duration::from_secs(900));
            let line = lines.iter().find(|l| l.contains("query-result")).unwrap().clone();
            digests.push(field(&line, "results").to_owned());
            table.push(line);
        }
        assert_eq!(
            digests[0], digests[1],
            "{word} {ranking}: the two indexes returned different results"
        );
    }
    println!(
        "\n| directory | word | ranking | hits | open ms | cold ms | warm median ms | warm p90 ms | 4 threads median ms | peak RSS MiB | cache MiB | load start | load end |"
    );
    println!(
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |"
    );
    for line in &table {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            field(line, "variant"),
            field(line, "word"),
            field(line, "ranking"),
            field(line, "hits"),
            field(line, "open_ms"),
            field(line, "cold_ms"),
            field(line, "warm_median_ms"),
            field(line, "warm_p90_ms"),
            field(line, "threads4_median_ms"),
            field(line, "peak_rss_mib"),
            field(line, "cache_mib"),
            field(line, "load_start").replace('_', " "),
            field(line, "load_end").replace('_', " "),
        );
    }
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().metadata().unwrap().len()).sum()
}

fn free_disk(path: &Path) -> String {
    let out =
        std::process::Command::new("df").args(["-h", path.to_str().unwrap()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .nth(1)
        .unwrap_or("")
        .split_whitespace()
        .nth(3)
        .unwrap_or("?")
        .to_owned()
}

fn load() -> String {
    common::load_averages().replace(' ', "_")
}

#[test]
#[ignore = "runs only as a child of the benchmark"]
fn child_build() {
    if !is_child() {
        return;
    }
    let dir = PathBuf::from(std::env::var("INDEX_DIR").unwrap());
    let variant = std::env::var("VARIANT").unwrap();
    let load_start = load();
    let started = Instant::now();
    let (schema, fields) = pages::schema();
    let index = Index::create(open_directory(&variant, &dir, 64), schema, IndexSettings::default())
        .unwrap();
    let mut writer: IndexWriter = index.writer_with_num_threads(2, 200_000_000).unwrap();
    writer.set_merge_policy(Box::new(tantivy::indexer::NoMergePolicy));
    pages::add_pages(&writer, &fields, 0..PAGES, 0);
    writer.commit().unwrap();
    let indexed = started.elapsed();
    let segments = index.searchable_segment_ids().unwrap();
    writer.merge(&segments).wait().unwrap();
    writer.garbage_collect_files().wait().unwrap();
    writer.wait_merging_threads().unwrap();
    println!(
        "build-result variant={variant} pages={PAGES} index_s={:.1} with_merge_s={:.1} bytes={} peak_rss_mib={:.0} load_start={load_start} load_end={}",
        indexed.as_secs_f64(),
        started.elapsed().as_secs_f64(),
        dir_size(&dir),
        common::peak_rss_mib(),
        load(),
    );
}

/// Runs the query and returns the page ids of the top 100 with their scores, best first.
fn run_query(searcher: &Searcher, word: &str, ranking: &str) -> Vec<(u64, f32)> {
    let (_, fields) = pages::schema();
    let query = pages::title_term(&fields, word);
    let hits: Vec<(f32, DocAddress)> = match ranking {
        "score" => searcher.search(&*query, &TopDocs::with_limit(100).order_by_score()).unwrap(),
        "score+recency" => {
            let collector = TopDocs::with_limit(100).tweak_score(|segment: &SegmentReader| {
                let last_visit =
                    segment.fast_fields().i64("pk_last_visit").unwrap().first_or_default_col(0);
                move |doc: DocId, score: Score| (score, last_visit.get_val(doc))
            });
            searcher
                .search(&*query, &collector)
                .unwrap()
                .into_iter()
                .map(|((score, _), doc)| (score, doc))
                .collect()
        }
        _ => unreachable!(),
    };
    let ids: Vec<_> = searcher
        .segment_readers()
        .iter()
        .map(|segment| segment.fast_fields().u64("pk_id").unwrap())
        .collect();
    let mut out: Vec<(u64, f32)> = hits
        .into_iter()
        .map(|(score, address)| {
            (ids[address.segment_ord as usize].first(address.doc_id).unwrap(), score)
        })
        .collect();
    if ranking == "score" {
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    }
    out
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[test]
#[ignore = "runs only as a child of the benchmark"]
fn child_query() {
    if !is_child() {
        return;
    }
    let dir = PathBuf::from(std::env::var("INDEX_DIR").unwrap());
    let variant = std::env::var("VARIANT").unwrap();
    let word = std::env::var("WORD").unwrap();
    let ranking = std::env::var("RANKING").unwrap();
    let cache_mib: usize = std::env::var("CACHE_MIB").unwrap().parse().unwrap();
    let load_start = load();

    let started = Instant::now();
    let encrypted = (variant == "encrypted")
        .then(|| EncryptedDirectory::open(&dir, &data_key(), cache_mib * 1024 * 1024).unwrap());
    let directory: Box<dyn Directory> = match &encrypted {
        Some(directory) => Box::new(directory.clone()),
        None => open_directory(&variant, &dir, cache_mib),
    };
    let index = Index::open(directory).unwrap();
    let searcher = Arc::new(index.reader().unwrap().searcher());
    let open = started.elapsed();

    let started = Instant::now();
    let results = run_query(&searcher, &word, &ranking);
    let cold = started.elapsed();

    let mut warm: Vec<f64> = (0..WARM_RUNS)
        .map(|_| {
            let started = Instant::now();
            assert_eq!(run_query(&searcher, &word, &ranking), results);
            millis(started.elapsed())
        })
        .collect();
    warm.sort_by(f64::total_cmp);

    let threads: Vec<_> = (0..4)
        .map(|_| {
            let searcher = searcher.clone();
            let word = word.clone();
            let ranking = ranking.clone();
            std::thread::spawn(move || {
                (0..WARM_RUNS)
                    .map(|_| {
                        let started = Instant::now();
                        run_query(&searcher, &word, &ranking);
                        millis(started.elapsed())
                    })
                    .collect::<Vec<f64>>()
            })
        })
        .collect();
    let mut concurrent: Vec<f64> = threads.into_iter().flat_map(|t| t.join().unwrap()).collect();
    concurrent.sort_by(f64::total_cmp);

    let hits = pages::count(&searcher, &*pages::title_term(&pages::schema().1, &word)).unwrap();
    // Compared across the two indexes: every score, and the ids above the 100th score. Ids that
    // tie at the 100th score depend on document order, which differs between two builds. With
    // the recency tiebreak no two pages tie, so every id is compared.
    let kth = if ranking == "score" { results.last().map_or(f32::MAX, |r| r.1) } else { f32::MIN };
    let digest: u64 = results.iter().fold(0xcbf2_9ce4_8422_2325, |h, (id, score)| {
        let id = if *score > kth { *id } else { 0 };
        (h ^ id ^ u64::from(score.to_bits())).wrapping_mul(0x100_0000_01b3)
    });
    let cache = encrypted.as_ref().map(|d| d.cache_stats());
    println!(
        "query-result variant={} word={word} ranking={ranking} hits={hits} open_ms={:.1} cold_ms={:.1} warm_median_ms={:.2} warm_p90_ms={:.2} threads4_median_ms={:.2} peak_rss_mib={:.0} cache_mib={} cache_hits={} cache_misses={} results={digest:016x} load_start={load_start} load_end={}",
        if variant == "encrypted" { "encrypted" } else { "MmapDirectory" },
        millis(open),
        millis(cold),
        warm[WARM_RUNS / 2],
        warm[WARM_RUNS * 9 / 10],
        concurrent[concurrent.len() / 2],
        common::peak_rss_mib(),
        cache.map_or("-".to_owned(), |c| format!("{:.1}", c.bytes as f64 / 1048576.0)),
        cache.map_or("-".to_owned(), |c| c.hits.to_string()),
        cache.map_or("-".to_owned(), |c| c.misses.to_string()),
        load(),
    );
}
