//! Responsibilities: resume real parsers into the real archive after committed URL batches.
//! Not responsible for: changing browser schemas or replacing storage with a mock.
//! Failure modes: old scan positions omit newly synced parents; Firefox/Safari replay
//! their whole URL phase; visits pass missing parents and become permanently lost.
use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, FirefoxPlaceRow,
    FirefoxPlacesFixture, FirefoxVisitRow, SafariHistoryFixture, SafariHistoryItemRow,
    SafariHistoryVisitRow,
};
use browser_history_parser::{
    HistoryBatchConsumer, ParsedUrl, ParsedVisit, SourceEvidenceChunk, UrlScan,
};
struct StopUrls<'a> {
    inner: ArchiveChunkConsumer<'a>,
    urls: usize,
    cancel: &'a CancelToken,
}
impl HistoryBatchConsumer for StopUrls<'_> {
    type Error = anyhow::Error;
    fn begin_urls(&mut self, scan: UrlScan) -> Result<()> {
        self.inner.begin_urls(scan)
    }
    fn urls(&mut self, rows: Vec<ParsedUrl>) -> Result<()> {
        self.urls += rows.len();
        self.inner.urls(rows)
    }
    fn visits(&mut self, rows: Vec<ParsedVisit>) -> Result<()> {
        self.inner.visits(rows)
    }
    fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<()> {
        self.inner.source_evidence(chunk)?;
        if self.urls >= 4 {
            self.cancel.cancel();
            self.cancel.check()?;
        }
        Ok(())
    }
    fn retain_source_evidence_in_report(&self) -> bool {
        false
    }
}
fn source(family: &str) -> ProfileSnapshot {
    let temp_dir = tempdir().unwrap();
    let history_path = temp_dir.path().join("History");
    let mut chrome = ChromiumHistoryFixture::new();
    let mut firefox = FirefoxPlacesFixture::new();
    let mut safari = SafariHistoryFixture::new();
    for id in (1..=7).map(|id| id * 2) {
        let url = format!("https://resume.test/{id}");
        let at = 1_800_000_000_000 + id;
        chrome = chrome
            .add_url(ChromiumUrlRow {
                id,
                url: url.clone(),
                title: None,
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: at,
                hidden: false,
            })
            .add_visit(ChromiumVisitRow {
                id,
                url_id: id,
                visit_time_unix_ms: at,
                from_visit: None,
                transition: None,
                visit_duration_micros: None,
                is_known_to_sync: false,
                visited_link_id: None,
                external_referrer_url: None,
                app_id: None,
            });
        firefox = firefox
            .add_place(FirefoxPlaceRow {
                id,
                url: url.clone(),
                title: None,
                visit_count: 1,
                hidden: false,
                last_visit_unix_ms: at,
            })
            .add_visit(FirefoxVisitRow {
                id,
                place_id: id,
                visit_time_unix_ms: at,
                from_visit: None,
                visit_type: None,
            });
        safari =
            safari.add_item(SafariHistoryItemRow { id, url }).add_visit(SafariHistoryVisitRow {
                id,
                history_item: id,
                title: None,
                visit_time_unix_ms: at,
                load_successful: None,
                http_non_get: None,
                synthesized: None,
                redirect_source: None,
                redirect_destination: None,
                origin: None,
                generation: None,
                attributes: None,
                score: None,
            });
    }
    match family {
        "chromium" => chrome.write(&history_path).unwrap(),
        "firefox" => firefox.write(&history_path).unwrap(),
        _ => safari.write(&history_path).unwrap(),
    };
    ProfileSnapshot {
        profile: test_context::profile(family, &format!("{family}:resume")),
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: vec![],
    }
}
#[test]
fn all_browsers_resume_url_rowids_without_reupserting_committed_prefixes() {
    for family in ["chromium", "firefox", "safari"] {
        for key in [None, Some([84; 32])] {
            let vault = crate::Vault::open_temp_with_key(key).unwrap();
            let source = source(family);
            let (context, batch) = test_context::seed_context(&vault, 42, &source.profile);
            let cancel = CancelToken::new();
            let inner = ArchiveChunkConsumer::new(
                &vault,
                context,
                Watermark::default(),
                &cancel,
                None,
                batch,
            );
            let mut stop = StopUrls { inner, urls: 0, cancel: &cancel };
            let result = match family {
                "chromium" => chromium::stream_history(
                    &HistoryDatabaseSet {
                        history_path: source.history_path.clone(),
                        favicons_path: None,
                    },
                    ChromiumReadCursor::default(),
                    2,
                    &mut stop,
                ),
                "firefox" => {
                    firefox::stream_history(&source.history_path, 0, 0, None, 2, &mut stop)
                }
                _ => safari::stream_history(&source.history_path, 0, 0, None, 2, &mut stop),
            }
            .map_err(stream_history_error_to_anyhow);
            drop(stop);
            assert!(result.is_err());
            let profile = source.profile.profile_id.clone();
            let watermark =
                vault.store().read(Db::Archive, move |s| load_watermark(s, &profile)).unwrap();
            assert_eq!(watermark.last_visit_id, 0);
            assert_eq!(watermark.url_scan.unwrap().after_id, 8);
            let run = super::super::write_runs::start(
                &vault,
                "backup",
                "manual",
                std::slice::from_ref(&source.profile.profile_id),
            )
            .unwrap();
            let summary = process_profile_snapshot(
                &vault,
                run,
                &source,
                false,
                true,
                &CancelToken::new(),
                None,
            )
            .unwrap();
            assert_eq!(summary.new_urls, 3);
            let stats: String = vault
                .store()
                .read(Db::Archive, move |s| {
                    s.query_one("SELECT stats_json FROM runs WHERE id=?1", [run], |r| Ok(r.get(0)?))
                })
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&stats).unwrap()["processedUrls"],
                3,
                "{family} must not re-upsert the four committed URLs"
            );
        }
    }
}
#[test]
fn chromium_resume_keeps_a_new_visit_to_an_older_synced_url() {
    let vault = crate::Vault::open_temp().unwrap();
    let source = source("chromium");
    let (context, batch) = test_context::seed_context(&vault, 42, &source.profile);
    let cancel = CancelToken::new();
    let inner =
        ArchiveChunkConsumer::new(&vault, context, Watermark::default(), &cancel, None, batch);
    let mut stop = StopUrls { inner, urls: 0, cancel: &cancel };
    assert!(
        chromium::stream_history(
            &HistoryDatabaseSet { history_path: source.history_path.clone(), favicons_path: None },
            ChromiumReadCursor::default(),
            2,
            &mut stop
        )
        .is_err()
    );
    drop(stop);
    let db = rusqlite::Connection::open(&source.history_path).unwrap();
    db.execute("INSERT INTO urls(id,url,title,visit_count,typed_count,last_visit_time,hidden) VALUES(1,'https://sync.test/older','Old time, new row',1,0,1,0)",[]).unwrap();
    db.execute("INSERT INTO visits(id,url,visit_time) VALUES(15,1,1)", []).unwrap();
    drop(db);
    let run = super::super::write_runs::start(
        &vault,
        "backup",
        "manual",
        std::slice::from_ref(&source.profile.profile_id),
    )
    .unwrap();
    process_profile_snapshot(&vault, run, &source, false, true, &CancelToken::new(), None).unwrap();
    vault.store().read(Db::Archive,|s| {
        assert_eq!(s.query_one("SELECT COUNT(*) FROM visits",(),|r|Ok(r.get::<i64>(0)?))?,8);
        assert_eq!(s.query_one("SELECT COUNT(*) FROM visits v JOIN urls u ON u.id=v.url_id WHERE u.url='https://sync.test/older'",(),|r|Ok(r.get::<i64>(0)?))?,1);
        Ok(())
    }).unwrap();
}
#[test]
fn a_visit_whose_url_row_is_missing_is_kept_with_its_address() {
    // The way this went wrong (test audit A3): the visit was skipped, a later visit in the batch
    // moved the cursor past it, and no backup read it again.
    let vault = crate::Vault::open_temp().unwrap();
    let profile = test_context::profile("chromium", "chromium:orphan");
    let (context, batch) = test_context::seed_context(&vault, 42, &profile);
    let cancel = CancelToken::new();
    let mut consumer =
        ArchiveChunkConsumer::new(&vault, context, Watermark::default(), &cancel, None, batch);
    let at = 1_800_000_000_000;
    let iso = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).unwrap().to_rfc3339();
    consumer
        .urls(vec![ParsedUrl {
            source_url_id: 2,
            url: "https://known.test/".into(),
            title: Some("Known".into()),
            visit_count: 1,
            typed_count: 0,
            last_visit_ms: at + 6,
            last_visit_iso: iso(at + 6),
            hidden: false,
            source_last_visit_marker: None,
        }])
        .unwrap();
    let visit = |id: i64, url_id: i64, url: &str, title: &str| ParsedVisit {
        source_visit_id: id,
        source_url_id: url_id,
        url: url.into(),
        title: Some(title.into()),
        visit_time_ms: at + id,
        visit_time_iso: iso(at + id),
        from_visit: None,
        transition: None,
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: None,
    };
    consumer
        .visits(vec![
            visit(4, 4, "https://missed.test/page", "Missed"),
            visit(6, 2, "https://known.test/", "Known"),
        ])
        .unwrap();
    let progress = consumer.finish().unwrap();
    assert_eq!(
        (progress.new_visits, progress.skipped_visits, progress.urls_from_visits),
        (2, 0, 1)
    );

    let profile_id = profile.profile_id.clone();
    let (visits, missed, cursor) = vault
        .store()
        .read(Db::Archive, move |s| {
            let visits =
                s.query_one("SELECT COUNT(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?;
            let missed = s.query_one("SELECT url FROM urls WHERE source_url_id = 4", (), |r| {
                Ok(r.get::<String>(0)?)
            })?;
            Ok((visits, missed, load_watermark(s, &profile_id)?.last_visit_id))
        })
        .unwrap();
    assert_eq!(visits, 2);
    assert_eq!(missed, "https://missed.test/page");
    assert_eq!(cursor, 6);
}
