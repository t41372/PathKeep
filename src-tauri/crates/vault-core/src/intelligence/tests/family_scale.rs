//! What query families cost at the size of a 14.4M-visit archive: the build adds every search
//! of a profile to `QueryFamilyAccumulator` one at a time, and an append first indexes every
//! wording the stored families hold (`resume`). A search is compared with every stored wording
//! that shares a token with it, so the cost grows with how many wordings share its words.
//!
//! Failure modes this watches: the per-search cost growing with the number of searches made
//! before it (each later chunk of searches slower than the one before), a common word that
//! makes every search containing it compare with tens of thousands of wordings, and an append
//! that spends seconds re-indexing families before it adds one search.
//!
//! Benchmark, not a check: `cargo test -p vault-core --release -- --ignored family_scale
//! --nocapture` prints `FAMILY_BENCH` lines. Searches are synthetic: 4% of 14.4M visits, the
//! share on the real-data copy (15,544 of 386,891, `ipc-performance.md` §11), all in one
//! profile on one engine, the worst case for one accumulator.

use super::super::intelligence_structural_build::QueryFamilyAccumulator;
use super::super::{SearchEventRecord, SearchQueryKind, normalize_query, query_token_set};
use crate::test_support::search_history::{
    Random, VOCABULARY, Zipf, made_up_word as word, synthetic_searches,
};
use std::time::Instant;

/// 4% of 14.4M visits.
const SEARCHES: usize = 576_000;
const CHUNK: usize = 48_000;

fn event(index: usize, query: &str) -> SearchEventRecord {
    SearchEventRecord {
        visit_id: index as i64 + 1,
        profile_id: "chrome:Default".into(),
        search_engine: "google".into(),
        raw_query: query.into(),
        normalized_query: normalize_query(query),
        query_kind: SearchQueryKind::Keyword,
        trail_id: None,
        // One search every 250 s: 576,000 searches span about 4.6 years, like the archive.
        visit_time_ms: 1_600_000_000_000 + index as i64 * 250_000,
    }
}

fn peak_rss_mb() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // The OS fills this plain structure when getrusage succeeds.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return 0;
    }
    let max = unsafe { usage.assume_init() }.ru_maxrss as u64;
    // Linux reports kilobytes, macOS bytes.
    if cfg!(target_os = "macos") { max >> 20 } else { max >> 10 }
}

#[test]
#[ignore = "benchmark: 576,000 synthetic searches; run in release with --nocapture"]
fn family_scale() {
    let started = Instant::now();
    let queries = synthetic_searches(SEARCHES);
    let distinct = queries.iter().collect::<std::collections::HashSet<_>>().len();
    println!(
        "FAMILY_BENCH generated searches={} distinct={distinct} elapsed_ms={}",
        queries.len(),
        started.elapsed().as_millis()
    );

    // A full build: every search of the profile, in time order.
    let mut families = QueryFamilyAccumulator::default();
    let started = Instant::now();
    let mut chunk = Instant::now();
    for (index, query) in queries.iter().enumerate() {
        families.add_event(&event(index, query));
        if (index + 1) % CHUNK == 0 {
            println!(
                "FAMILY_BENCH build searches={} families={} chunk_ms={} elapsed_ms={}",
                index + 1,
                families.families().len(),
                chunk.elapsed().as_millis(),
                started.elapsed().as_millis()
            );
            chunk = Instant::now();
        }
    }
    let built = families.finish();
    let wordings = built.iter().map(|family| family.queries.len()).sum::<usize>();
    let largest = built.iter().map(|family| family.queries.len()).max().unwrap_or(0);
    println!(
        "FAMILY_BENCH build total_ms={} families={} wordings={wordings} largest_family_wordings={largest} peak_rss_mb={}",
        started.elapsed().as_millis(),
        built.len(),
        peak_rss_mb()
    );
    // Hubs: the largest families, and how much of the history families begun by one word hold.
    let mut sizes = built
        .iter()
        .map(|family| {
            (family.member_count, family.queries.len(), query_token_set(&family.anchor_query).len())
        })
        .collect::<Vec<_>>();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    for (members, wordings, anchor_tokens) in sizes.iter().take(5) {
        println!(
            "FAMILY_BENCH largest members={members} wordings={wordings} first_search_tokens={anchor_tokens}"
        );
    }
    let one_word =
        sizes.iter().filter(|(.., tokens)| *tokens == 1).map(|(members, ..)| members).sum::<i64>();
    println!(
        "FAMILY_BENCH one_word_first_searches families={} searches={one_word} of={SEARCHES}",
        sizes.iter().filter(|(.., tokens)| *tokens == 1).count()
    );

    // An append: index the stored families again, then add a day of searches.
    let started = Instant::now();
    let mut resumed =
        QueryFamilyAccumulator::resume(super::super::InsightsZone::system(), built, None);
    println!("FAMILY_BENCH resume elapsed_ms={}", started.elapsed().as_millis());
    let started = Instant::now();
    for (index, query) in queries.iter().take(40).enumerate() {
        resumed.add_event(&event(SEARCHES + index, &format!("{query} {}", word(index))));
    }
    println!("FAMILY_BENCH append searches=40 elapsed_ms={}", started.elapsed().as_millis());

    // A word in every search: 30,000 searches that share only it, then 10,000 more like them.
    let mut random = Random(7);
    let words = Zipf::new(VOCABULARY);
    let mut common = QueryFamilyAccumulator::default();
    let started = Instant::now();
    for index in 0..30_000 {
        let query = format!(
            "python {} {}",
            word(1_000 + words.sample(&mut random)),
            word(1_000 + words.sample(&mut random))
        );
        common.add_event(&event(index, &query));
    }
    println!(
        "FAMILY_BENCH common_word seeded=30000 families={} elapsed_ms={}",
        common.families().len(),
        started.elapsed().as_millis()
    );
    let started = Instant::now();
    for index in 0..10_000 {
        let query = format!("python {}", word(60_000 + index));
        common.add_event(&event(30_000 + index, &query));
    }
    println!(
        "FAMILY_BENCH common_word searches=10000 elapsed_ms={} per_search_us={}",
        started.elapsed().as_millis(),
        started.elapsed().as_micros() / 10_000
    );
}
