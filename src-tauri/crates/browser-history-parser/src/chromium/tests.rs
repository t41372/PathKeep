//! Responsibilities: prove the Chromium parser against small staged History and Favicons files.
//! Not responsible for: browser discovery, live-file copying or archive ingest.
//! Failure modes: a staged table's rows left out, a URL behind the cursor whose new visit is
//! dropped, a source without `urls` or `visits` accepted, a Favicons file missing a table failing
//! the whole parse, evidence kept in the report after the consumer took it, or a corrupt
//! timestamp whose number and date disagree.
use super::*;
use std::convert::Infallible;
use tempfile::tempdir;

#[derive(Default)]
struct NonRetainingEvidenceSink {
    urls: usize,
    visits: usize,
    downloads: usize,
    search_terms: usize,
    favicons: usize,
    url_batches: Vec<usize>,
    visit_batches: Vec<usize>,
    download_batches: Vec<usize>,
    search_term_batches: Vec<usize>,
    favicon_batches: Vec<usize>,
    source_evidence_chunks: usize,
    search_evidence: usize,
    navigation_evidence: usize,
    engagement_evidence: usize,
    context_evidence: usize,
    native_entities: usize,
}

impl HistoryBatchConsumer for NonRetainingEvidenceSink {
    type Error = Infallible;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.url_batches.push(batch.len());
        self.urls += batch.len();
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.visit_batches.push(batch.len());
        self.visits += batch.len();
        Ok(())
    }

    fn downloads(&mut self, batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
        self.download_batches.push(batch.len());
        self.downloads += batch.len();
        Ok(())
    }

    fn search_terms(&mut self, batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
        self.search_term_batches.push(batch.len());
        self.search_terms += batch.len();
        Ok(())
    }

    fn favicons(&mut self, batch: Vec<ParsedFavicon>) -> Result<(), Self::Error> {
        self.favicon_batches.push(batch.len());
        self.favicons += batch.len();
        Ok(())
    }

    fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<(), Self::Error> {
        self.source_evidence_chunks += 1;
        self.search_evidence += chunk.typed_evidence.search.len();
        self.navigation_evidence += chunk.typed_evidence.navigation.len();
        self.engagement_evidence += chunk.typed_evidence.engagement.len();
        self.context_evidence += chunk.typed_evidence.context.len();
        self.native_entities += chunk.native_entities.len();
        Ok(())
    }

    fn retain_source_evidence_in_report(&self) -> bool {
        false
    }
}

fn write_history_fixture(path: &Path) {
    let connection = Connection::open(path).expect("open history fixture");
    connection
        .execute_batch(
            "CREATE TABLE urls (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER NOT NULL,
               typed_count INTEGER NOT NULL,
               last_visit_time INTEGER NOT NULL,
               hidden INTEGER NOT NULL
             );
             CREATE TABLE visits (
               id INTEGER PRIMARY KEY,
               url INTEGER NOT NULL,
               visit_time INTEGER NOT NULL,
               from_visit INTEGER,
               transition INTEGER,
               visit_duration INTEGER,
               is_known_to_sync INTEGER,
               visited_link_id INTEGER,
               external_referrer_url TEXT,
               app_id TEXT
             );
             CREATE TABLE downloads (
               id INTEGER PRIMARY KEY,
               guid TEXT,
               current_path TEXT,
               target_path TEXT,
               start_time INTEGER,
               received_bytes INTEGER,
               total_bytes INTEGER,
               state INTEGER,
               mime_type TEXT,
               original_mime_type TEXT
             );
             CREATE TABLE keyword_search_terms (
               keyword_id INTEGER,
               url_id INTEGER,
               term TEXT,
               normalized_term TEXT
             );",
        )
        .expect("create history schema");
    connection
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                1_i64,
                "https://example.com/article",
                "Example Article",
                5_i64,
                2_i64,
                13_000_000_000_000_000_i64,
                0_i64
            ],
        )
        .expect("insert url");
    connection
        .execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                10_i64,
                1_i64,
                13_000_000_000_500_000_i64,
                Option::<i64>::None,
                805_306_368_i64,
                4000_i64,
                1_i64,
                7_i64,
                "https://referrer.example.com",
                "com.example.browser"
            ],
        )
        .expect("insert visit");
    connection
        .execute(
            "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                4_i64,
                "download-guid",
                "/tmp/example.part",
                "/tmp/example.zip",
                13_000_000_001_000_000_i64,
                128_i64,
                256_i64,
                1_i64,
                "application/zip",
                "application/octet-stream"
            ],
        )
        .expect("insert download");
    connection
        .execute(
            "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
             VALUES (?1, ?2, ?3, ?4)",
            params![2_i64, 1_i64, "PathKeep", "pathkeep"],
        )
        .expect("insert search term");
}

fn write_favicons_fixture(path: &Path) {
    let connection = Connection::open(path).expect("open favicons fixture");
    connection
        .execute_batch(
            "CREATE TABLE favicons (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               icon_type INTEGER
             );
             CREATE TABLE icon_mapping (
               page_url TEXT NOT NULL,
               icon_id INTEGER NOT NULL
             );
             CREATE TABLE favicon_bitmaps (
               icon_id INTEGER NOT NULL,
               width INTEGER,
               height INTEGER,
               last_updated INTEGER,
               image_data BLOB
             );",
        )
        .expect("create favicons schema");
    connection
        .execute(
            "INSERT INTO favicons (id, url, icon_type) VALUES (?1, ?2, ?3)",
            params![3_i64, "https://example.com/favicon.ico", 1_i64],
        )
        .expect("insert favicon");
    connection
        .execute(
            "INSERT INTO icon_mapping (page_url, icon_id) VALUES (?1, ?2)",
            params!["https://example.com/article", 3_i64],
        )
        .expect("insert mapping");
    connection
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![3_i64, 32_i64, 32_i64, 13_000_000_002_000_000_i64, vec![1_u8, 2, 3]],
        )
        .expect("insert bitmap");
}

fn write_partial_favicons_fixture(path: &Path) {
    let connection = Connection::open(path).expect("open partial favicons fixture");
    connection
        .execute_batch(
            "CREATE TABLE favicons (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               icon_type INTEGER
             );
             CREATE TABLE favicon_bitmaps (
               icon_id INTEGER NOT NULL,
               width INTEGER,
               height INTEGER,
               last_updated INTEGER,
               image_data BLOB
             );",
        )
        .expect("create partial favicons schema");
    connection
        .execute(
            "INSERT INTO favicons (id, url, icon_type) VALUES (?1, ?2, ?3)",
            params![3_i64, "https://example.com/favicon.ico", 1_i64],
        )
        .expect("insert partial favicon");
    connection
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![3_i64, 32_i64, 32_i64, 13_000_000_002_000_000_i64, vec![1_u8, 2, 3]],
        )
        .expect("insert partial bitmap");
}

#[test]
fn parse_history_includes_urls_whose_last_visit_time_is_below_cursor_when_new_visits_arrive() {
    // Models the long-tail revisit case: a Zhihu-style answer page whose
    // urls.last_visit_time was set in a previous import (so it's below
    // the next URL cursor), but a fresh visit arrives whose id is above
    // the visit cursor. The URL pass must still pick the URL up so the
    // visit pass can attribute the visit to it instead of bumping
    // skipped_visits++.
    let directory = tempdir().expect("tempdir");
    let history_path = directory.path().join("History");
    let connection = Connection::open(&history_path).expect("open fixture");
    connection
        .execute_batch(
            "CREATE TABLE urls (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER NOT NULL,
               typed_count INTEGER NOT NULL,
               last_visit_time INTEGER NOT NULL,
               hidden INTEGER NOT NULL
             );
             CREATE TABLE visits (
               id INTEGER PRIMARY KEY,
               url INTEGER NOT NULL,
               visit_time INTEGER NOT NULL,
               from_visit INTEGER,
               transition INTEGER,
               visit_duration INTEGER,
               is_known_to_sync INTEGER,
               visited_link_id INTEGER,
               external_referrer_url TEXT,
               app_id TEXT
             );",
        )
        .expect("create minimal schema");
    // URL's last_visit_time intentionally sits *below* the URL cursor
    // we'll pass in. Pre-fix, this URL would not appear in the URL pass.
    connection
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                42_i64,
                "https://www.zhihu.com/question/12345/answer/67890",
                "如何理解...",
                3_i64,
                0_i64,
                13_000_000_000_000_000_i64,
                0_i64
            ],
        )
        .expect("insert zhihu url");
    // Visit id 99 lives ABOVE the visit cursor (50) we'll pass in.
    connection
        .execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![99_i64, 42_i64, 13_000_000_000_900_000_i64, Option::<i64>::None, 0_i64, 1000_i64, 1_i64, 0_i64, Option::<&str>::None, Option::<&str>::None],
        )
        .expect("insert new visit");

    // URL cursor at 13_000_000_001_000_000 (above the URL's last_visit_time),
    // visit cursor at 50 (below the new visit id).
    let parsed = parse_history(
        &HistoryDatabaseSet { history_path, favicons_path: None },
        ChromiumReadCursor {
            after_search_url_time: 0,
            url_scan: None,
            after_url_last_visit_time: 13_000_000_001_000_000_i64,
            after_visit_id: 50_i64,
            after_download_id: 0,
            after_favicon_last_updated: 0,
        },
    )
    .expect("parse history with split cursors");

    assert_eq!(parsed.urls.len(), 1, "url must be widened in via the visit cursor");
    assert_eq!(parsed.urls[0].url, "https://www.zhihu.com/question/12345/answer/67890");
    assert_eq!(parsed.visits.len(), 1, "new visit must land, not be skipped");
    assert_eq!(parsed.visits[0].source_visit_id, 99);
    assert_eq!(parsed.visits[0].source_url_id, 42);
}

#[test]
fn parse_history_returns_incremental_rows_from_provided_paths() {
    let directory = tempdir().expect("tempdir");
    let history_path = directory.path().join("History");
    let favicons_path = directory.path().join("Favicons");
    write_history_fixture(&history_path);
    write_favicons_fixture(&favicons_path);

    let parsed = parse_history(
        &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
        ChromiumReadCursor::default(),
    )
    .expect("parse history");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.downloads.len(), 1);
    assert_eq!(parsed.search_terms.len(), 1);
    assert_eq!(parsed.favicons.len(), 1);
    assert_eq!(parsed.urls[0].url, "https://example.com/article");
    assert!(!parsed.urls[0].hidden);
    assert_eq!(parsed.search_terms[0].normalized_term, "pathkeep");
    assert!(parsed.visits[0].is_known_to_sync);
    assert_eq!(parsed.favicons[0].width, 32);
}

#[test]
fn stream_history_can_move_source_evidence_out_of_the_returned_report() {
    let directory = tempdir().expect("tempdir");
    let history_path = directory.path().join("History");
    let favicons_path = directory.path().join("Favicons");
    write_history_fixture(&history_path);
    write_favicons_fixture(&favicons_path);
    let mut sink = NonRetainingEvidenceSink::default();

    let streamed = stream_history(
        &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
        ChromiumReadCursor::default(),
        1,
        &mut sink,
    )
    .expect("stream history");

    assert_eq!(sink.urls, 1);
    assert_eq!(sink.visits, 1);
    assert_eq!(sink.downloads, 1);
    assert_eq!(sink.search_terms, 1);
    assert!(sink.source_evidence_chunks >= 4);
    assert_eq!(sink.search_evidence, 1);
    assert_eq!(sink.navigation_evidence, 1);
    assert_eq!(sink.engagement_evidence, 1);
    assert_eq!(sink.context_evidence, 3);
    assert!(sink.native_entities >= 4);
    assert!(streamed.native_entities.is_empty());
    assert!(streamed.typed_evidence.search.is_empty());
    assert!(streamed.typed_evidence.navigation.is_empty());
    assert!(streamed.typed_evidence.engagement.is_empty());
    assert!(streamed.typed_evidence.context.is_empty());
    assert!(
        streamed
            .capability_snapshot
            .items
            .iter()
            .any(|item| { item.key == "search.native_terms" && item.populated_rows == 1 })
    );
}

#[test]
fn parse_history_requires_urls_and_visits_tables() {
    let directory = tempdir().expect("tempdir");
    let history_path = directory.path().join("History");
    let connection = Connection::open(&history_path).expect("open fixture");
    connection
        .execute(
            "CREATE TABLE urls (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER NOT NULL,
               typed_count INTEGER NOT NULL,
               last_visit_time INTEGER NOT NULL,
               hidden INTEGER NOT NULL
             )",
            [],
        )
        .expect("create urls table");

    let error = parse_history(
        &HistoryDatabaseSet { history_path, favicons_path: None },
        ChromiumReadCursor::default(),
    )
    .expect_err("missing visits should fail");
    assert!(matches!(error, ParseError::MissingTable { table: "visits" }));
}

#[test]
fn parse_history_skips_favicons_when_support_tables_are_missing() {
    let directory = tempdir().expect("tempdir");
    let history_path = directory.path().join("History");
    let favicons_path = directory.path().join("Favicons");
    write_history_fixture(&history_path);
    write_partial_favicons_fixture(&favicons_path);

    let parsed = parse_history(
        &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
        ChromiumReadCursor::default(),
    )
    .expect("parse history");

    assert!(parsed.favicons.is_empty());
    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 1);
}

#[test]
fn chrome_time_helpers_clamp_invalid_values_and_keep_iso_stable() {
    assert_eq!(chrome_time_to_unix_ms(i64::MIN), 0);
    assert_eq!(chrome_time_to_iso(i64::MIN), "1970-01-01T00:00:00+00:00");
    assert!(chrome_time_to_iso(13_000_000_000_000_000_i64).starts_with("2012-"));
    // Far-future / corrupt micros clamp to the representable ceiling so the
    // numeric *_ms value and the ISO string agree (no year-294071 number
    // paired with a silent 1970 ISO fallback).
    assert_eq!(chrome_time_to_unix_ms(i64::MAX), crate::types::MAX_VALID_UNIX_MS);
    assert!(chrome_time_to_iso(i64::MAX).starts_with("9999-"));
}
