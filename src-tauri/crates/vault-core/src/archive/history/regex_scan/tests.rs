//! Tests for the chunked regex scan, one per failure mode in the module header. The budget is set
//! in rows instead of time so every chunk edge is deterministic.

use super::*;
use crate::archive::history::grouped::tests::{Archive, CHROME, FIREFOX};

fn rows(rows: usize) -> ScanBudget {
    ScanBudget { time: None, rows }
}

const UNBOUNDED: ScanBudget = ScanBudget { time: None, rows: usize::MAX };

struct Scan<'a> {
    archive: &'a Archive,
    pattern: &'a str,
    filters: PageFilters,
    sort: &'static str,
    grouped: bool,
}

impl<'a> Scan<'a> {
    fn new(archive: &'a Archive, pattern: &'a str) -> Self {
        Self {
            archive,
            pattern,
            filters: PageFilters {
                profile_id: None,
                browser_kind: None,
                domain_pattern: None,
                start_time_ms: None,
                end_time_ms: None,
            },
            sort: "newest",
            grouped: false,
        }
    }

    /// One chunk, on a fresh connection like every real request.
    fn chunk(
        &self,
        limit: usize,
        cursor: Option<&str>,
        budget: ScanBudget,
    ) -> HistoryQueryResponse {
        let connection =
            open_archive_connection(&self.archive.paths, &self.archive.config, None).expect("open");
        prepare_advanced_search_filters(&connection, &ParsedHistorySearchQuery::default())
            .expect("filter tables");
        let regex = RegexBuilder::new(self.pattern).case_insensitive(true).build().expect("regex");
        scan_regex(
            &connection,
            &RegexScan {
                regex: &regex,
                filters: &self.filters,
                sort: self.sort,
                limit,
                cursor,
                requested_page: None,
                grouped: self.grouped,
            },
            budget,
        )
        .expect("scan")
    }

    /// Every chunk until the scan completes.
    fn walk(&self, limit: usize, budget: ScanBudget) -> Vec<HistoryQueryResponse> {
        let mut chunks = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let chunk = self.chunk(limit, cursor.as_deref(), budget);
            cursor = chunk.next_cursor.clone();
            chunks.push(chunk);
            if cursor.is_none() {
                break chunks;
            }
            assert!(chunks.len() < 10_000, "the scan never completes");
        }
    }
}

fn ids(chunks: &[HistoryQueryResponse]) -> Vec<i64> {
    chunks.iter().flat_map(|chunk| chunk.items.iter().map(|item| item.id)).collect()
}

/// Rows of the same page from different chunks added up, in first-seen order.
fn merged_pages(chunks: &[HistoryQueryResponse]) -> Vec<(String, u64, i64)> {
    let mut pages: Vec<(String, u64, i64)> = Vec::new();
    for item in chunks.iter().flat_map(|chunk| &chunk.items) {
        let count = item.visit_count.unwrap_or(0);
        match pages.iter_mut().find(|(url, _, _)| *url == item.url) {
            Some(page) => page.1 += count,
            None => pages.push((item.url.clone(), count, item.visit_time)),
        }
    }
    pages
}

/// Thirty unrelated pages visited at 1_000 to 30_000, plus three `story` pages, the oldest of them
/// visited before everything else.
fn archive_with_an_old_match() -> Archive {
    let archive = Archive::new();
    for id in 1..=30 {
        archive.url(id, CHROME, &format!("https://filler.test/{id}"), "filler", &[1_000 * id]);
    }
    archive
        .url(31, FIREFOX, "https://news.test/story?id=1", "Story one", &[500, 12_500, 25_500])
        .url(32, CHROME, "https://news.test/story?id=2", "Story two", &[7_500])
        .url(33, CHROME, "https://news.test/story?id=1", "Story one", &[18_500, 300])
        .url(34, CHROME, "https://news.test/old", "An old STORY", &[100]);
    archive
}

#[test]
fn a_match_older_than_the_first_chunk_is_found_by_continuing() {
    let archive = Archive::new();
    for id in 1..=30 {
        archive.url(id, CHROME, &format!("https://filler.test/{id}"), "filler", &[1_000 * id]);
    }
    archive.url(31, CHROME, "https://old.test/needle", "Needle", &[10]);
    let scan = Scan::new(&archive, "needle");

    let first = scan.chunk(100, None, rows(10));
    assert!(first.items.is_empty());
    let progress = first.regex_scan.expect("progress");
    assert!(!progress.complete);
    assert_eq!(progress.scanned_to_ms, Some(21_000), "ten newest visits scanned");
    assert!(first.has_next && !first.total_exact);

    let chunks = scan.walk(100, rows(10));
    assert_eq!(chunks.len(), 4, "31 visits, ten per chunk");
    let found = chunks.iter().flat_map(|chunk| &chunk.items).collect::<Vec<_>>();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].url, "https://old.test/needle");
    let last = chunks.last().expect("last chunk");
    assert!(last.regex_scan.expect("progress").complete);
    assert!(!last.has_next && last.next_cursor.is_none());
}

#[test]
fn chunk_edges_neither_repeat_nor_skip_a_row() {
    let archive = archive_with_an_old_match();
    for sort in ["newest", "oldest"] {
        let mut scan = Scan::new(&archive, "story");
        scan.sort = sort;
        let whole = scan.chunk(1_000, None, UNBOUNDED);
        assert_eq!(whole.items.len(), 7, "{sort}: every story visit");
        for budget in [1, 2, 3, 7, 40] {
            for limit in [1, 2, 5, 100] {
                let walked = scan.walk(limit, rows(budget));
                assert_eq!(
                    ids(&walked),
                    ids(std::slice::from_ref(&whole)),
                    "{sort}: budget {budget}, limit {limit}"
                );
            }
        }

        scan.grouped = true;
        let whole = scan.chunk(1_000, None, UNBOUNDED);
        assert_eq!(whole.items.len(), 3, "{sort}: three story pages");
        let expected = merged_pages(std::slice::from_ref(&whole));
        for budget in [1, 2, 3, 7, 40] {
            for limit in [1, 2, 100] {
                let walked = scan.walk(limit, rows(budget));
                assert_eq!(
                    merged_pages(&walked),
                    expected,
                    "{sort}: budget {budget}, limit {limit}"
                );
            }
        }
    }
}

#[test]
fn a_count_is_exact_only_once_one_response_covered_everything() {
    let archive = archive_with_an_old_match();
    let mut scan = Scan::new(&archive, "story");
    scan.grouped = true;

    let whole = scan.chunk(100, None, UNBOUNDED);
    assert!(whole.total_exact);
    assert_eq!((whole.total, whole.total_visits), (3, Some(7)));
    assert!(whole.regex_scan.expect("progress").complete);

    let chunks = scan.walk(100, rows(10));
    assert!(chunks.len() > 1);
    for chunk in &chunks {
        assert!(!chunk.total_exact, "a chunk of a longer scan never claims an exact count");
    }
    let (done, partial) = chunks.split_last().expect("chunks");
    assert!(done.regex_scan.expect("progress").complete);
    assert!(partial.iter().all(|chunk| !chunk.regex_scan.expect("progress").complete));

    // A full page stops the scan early, which is not coverage either.
    let first_page = scan.chunk(1, None, UNBOUNDED);
    assert!(!first_page.total_exact && !first_page.regex_scan.expect("progress").complete);
}

#[test]
fn an_abandoned_scan_leaves_nothing_behind() {
    let archive = archive_with_an_old_match();
    let scan = Scan::new(&archive, "story");

    // The chunk stops at its budget; the cursor alone resumes it, on another connection, with no
    // state kept anywhere else. Not asking for the next chunk is all it takes to stop.
    let first = scan.chunk(100, None, rows(5));
    assert_eq!(first.regex_scan.expect("progress").scanned_to_ms, Some(26_000));
    let cursor = first.next_cursor.clone().expect("more to scan");
    let resumed = scan.chunk(100, Some(&cursor), UNBOUNDED);
    let again = scan.chunk(100, Some(&cursor), UNBOUNDED);
    assert_eq!(ids(&[resumed]), ids(&[again]), "the same cursor resumes the same way");

    // A spent budget still moves forward by one visit, so the scan cannot stall.
    let zero = scan.chunk(100, None, ScanBudget { time: Some(Duration::ZERO), rows: 0 });
    assert_eq!(zero.regex_scan.expect("progress").scanned_to_ms, Some(30_000));
    assert!(zero.next_cursor.is_some());
}

#[test]
fn filters_bound_the_scan_before_the_regex() {
    let archive = archive_with_an_old_match();
    let mut scan = Scan::new(&archive, "story");
    scan.filters.end_time_ms = Some(10_000);
    scan.filters.start_time_ms = Some(200);
    let whole = scan.chunk(100, None, UNBOUNDED);
    assert_eq!(
        whole.items.iter().map(|item| item.visit_time).collect::<Vec<_>>(),
        [7_500, 500, 300],
        "only visits inside the dates, newest first"
    );
    assert!(whole.total_exact, "the date range was covered whole");

    scan.filters.start_time_ms = None;
    scan.filters.end_time_ms = None;
    scan.filters.browser_kind = Some("firefox".into());
    let firefox = scan.chunk(100, None, UNBOUNDED);
    assert!(firefox.items.iter().all(|item| item.profile_id == "firefox:main"));
    assert_eq!(firefox.items.len(), 3);
}

#[test]
fn the_dialect_is_the_rust_regex_crate() {
    let archive = archive_with_an_old_match();
    archive.index();
    let search = |pattern: &str| {
        list_history(
            &archive.paths,
            &archive.config,
            None,
            HistoryQuery {
                q: Some(pattern.to_string()),
                regex_mode: Some(true),
                ..HistoryQuery::default()
            },
        )
    };
    let look_around = search("story(?=one)").expect_err("look-around is not Rust regex");
    assert!(format!("{look_around:#}").contains("invalid regex pattern"));
    let backreference = search(r"(s)\1").expect_err("backreferences are not Rust regex");
    assert!(format!("{backreference:#}").contains("invalid regex pattern"));
    let classes = search(r"story\?id=\d$").expect("Rust classes work");
    assert_eq!(classes.items.len(), 6, "case-insensitive, URL or title");
}

#[test]
fn the_scan_walks_the_time_index_without_sorting() {
    // A sort would read every visit inside the filters before the first chunk could return.
    let archive = Archive::new();
    let connection =
        open_archive_connection(&archive.paths, &archive.config, None).expect("open archive");
    for sql in [REGEX_SCAN_NEWEST_SQL, REGEX_SCAN_OLDEST_SQL] {
        let mut statement =
            connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).expect("plan statement");
        let plan = statement
            .query_map(
                named_params! {
                    ":profileId": Option::<String>::None,
                    ":browserKind": Option::<String>::None,
                    ":domainPattern": Option::<String>::None,
                    ":startTimeMs": i64::MIN,
                    ":endTimeMs": i64::MAX,
                    ":cursorVisitTime": i64::MAX,
                    ":cursorId": i64::MAX,
                },
                |row| row.get::<_, String>(3),
            )
            .expect("plan rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("plan text")
            .join("\n");
        assert!(
            plan.contains("idx_visits_visible_time_id") && !plan.contains("TEMP B-TREE"),
            "the regex scan must walk the time index:\n{plan}"
        );
    }
}
