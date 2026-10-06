//! Real archive regressions for export failure modes documented in progress.rs.
//! Responsibilities: real artifact, cancellation, paging and write-buffer contracts.
//! Not responsible for: search recall or transport.
//! Failure modes: missed tied rows, cancelled artifacts published, shared cancellation, unbounded buffers.

use super::*;
use crate::models::ExportState;
use std::{io, thread, time::Instant};

pub(crate) fn seeded_archive(rows: usize, key: Option<[u8; 32]>) -> crate::Vault {
    let vault = crate::Vault::open_temp_with_key(key).unwrap();
    vault.store().write(vault_store::Db::Archive,vault_store::Priority::Batch,move |s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,enabled,profile_key) VALUES(1,'chrome','Default','/fixture','',1,'chrome:Default')",())?;
        s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,'https://example.com/?a=1&b=2','https://example.com/?a=1&b=2')",())?;
        s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) VALUES(1,1,'https://example.com/?a=1&b=2','Title <中文>',0,'',0,'',1,0)",())?;
        s.execute("WITH RECURSIVE ids(id) AS (VALUES(1) UNION ALL SELECT id+1 FROM ids WHERE id<?1) INSERT INTO visits(url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) SELECT 1,CAST(id AS TEXT),1700000000000+id/3,'',1,0 FROM ids",[rows as i64])?;
        s.execute("UPDATE archive_totals SET value=?1 WHERE name='visits'",[rows as i64])?;
        Ok(())
    }).unwrap();
    vault
}

fn request(id: &str, query: HistoryQuery, format: ExportFormat) -> ExportRequest {
    ExportRequest { export_id: Some(id.into()), query, format }
}

fn poll_until(id: &str, ready: impl Fn(&crate::ExportProgress) -> bool) -> crate::ExportProgress {
    let started = Instant::now();
    loop {
        if let Some(progress) = get_export_progress(id) {
            if ready(&progress) {
                return progress;
            }
        }
        assert!(started.elapsed() < Duration::from_secs(5), "progress did not arrive: {id}");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn cancellation_during_writing_removes_temp_and_isolates_other_exports() {
    for key in [None, Some([76; 32])] {
        let vault = seeded_archive(100_000, key);
        let paths = vault.paths();
        thread::scope(|scope| {
            let cancel = scope.spawn(|| {
                export_history(
                    &vault,
                    request("cancel-writing", HistoryQuery::default(), ExportFormat::Jsonl),
                )
            });
            let keep = scope.spawn(|| {
                export_history(
                    &vault,
                    request("keep-writing", HistoryQuery::default(), ExportFormat::Jsonl),
                )
            });
            let progress = poll_until("cancel-writing", |progress| {
                progress.state == ExportState::Running && progress.rows_written >= 128
            });
            assert_eq!(progress.state, ExportState::Running);
            assert_eq!(progress.total_rows, Some(100_000));
            let started = Instant::now();
            assert!(cancel_export("cancel-writing"));
            let error = cancel.join().unwrap().unwrap_err();
            assert!(error.downcast_ref::<ExportCancelled>().is_some(), "{error:#}");
            assert!(started.elapsed() < Duration::from_secs(1));
            let result = keep.join().unwrap().unwrap();
            assert_eq!(result.count, 100_000);
            let files: Vec<_> = fs::read_dir(&paths.exports_dir).unwrap().collect();
            assert_eq!(files.len(), 1, "cancel must leave neither a temp nor a destination");
            let done = get_export_progress("keep-writing").unwrap();
            assert_eq!(done.state, ExportState::Done);
            assert_eq!(done.rows_written, 100_000);
            assert_eq!(done.bytes_written, fs::metadata(&result.path).unwrap().len());
            assert!(done.error.is_none());
            assert!(!cancel_export("keep-writing"));
            assert_eq!(
                get_export_progress("cancel-writing").unwrap().state,
                ExportState::Cancelled
            );
        });
    }
}

#[test]
fn failures_at_setup_and_during_query_always_become_terminal() {
    for key in [None, Some([76; 32])] {
        let vault = seeded_archive(4, key);
        let paths = vault.paths();
        fs::create_dir_all(&paths.exports_dir).unwrap();
        vault.store().close().unwrap();
        assert!(
            export_history(
                &vault,
                request("failed-query", HistoryQuery::default(), ExportFormat::Jsonl)
            )
            .is_err()
        );
        let progress = get_export_progress("failed-query").unwrap();
        assert_eq!(progress.state, ExportState::Failed);
        assert_eq!(fs::read_dir(&paths.exports_dir).unwrap().count(), 0);
        let vault = seeded_archive(4, key);
        let paths = vault.paths();
        fs::create_dir_all(paths.exports_dir.parent().unwrap()).unwrap();
        fs::write(&paths.exports_dir, b"not a directory").unwrap();
        assert!(
            export_history(
                &vault,
                request("failed-setup", HistoryQuery::default(), ExportFormat::Html)
            )
            .is_err()
        );
        assert_eq!(get_export_progress("failed-setup").unwrap().state, ExportState::Failed);
        assert!(!cancel_export("failed-setup"));
    }
}

#[test]
fn streamed_output_matches_previous_page_walk_for_all_formats_and_filters() {
    for key in [None, Some([76; 32])] {
        let vault = seeded_archive(1_205, key);
        for query in [
            HistoryQuery::default(),
            HistoryQuery { sort: Some("oldest".into()), ..HistoryQuery::default() },
            HistoryQuery {
                profile_id: Some("chrome:Default".into()),
                domain: Some("example.com".into()),
                start_time_ms: Some(1700000000100),
                ..HistoryQuery::default()
            },
            HistoryQuery { domain: Some("no-matches".into()), ..HistoryQuery::default() },
        ] {
            for format in [
                ExportFormat::Jsonl,
                ExportFormat::Html,
                ExportFormat::Markdown,
                ExportFormat::Text,
            ] {
                // The old algorithm: independent public list calls, page/limit reset, render each row.
                let mut old_query = query.clone();
                old_query.limit = Some(EXPORT_PAGE_SIZE);
                old_query.page = None;
                old_query.cursor = None;
                old_query.include_total = Some(false);
                let mut expected = Vec::new();
                let mut old = ExportWriter::new(&mut expected, &format).unwrap();
                loop {
                    let page = crate::list_history(&vault, old_query.clone()).unwrap();
                    for item in &page.items {
                        old.item(item).unwrap();
                    }
                    let Some(cursor) = page.next_cursor else {
                        break;
                    };
                    old_query.cursor = Some(cursor);
                }
                let count = old.finish().unwrap();
                let result = export_history(
                    &vault,
                    ExportRequest { export_id: None, query: query.clone(), format },
                )
                .unwrap();
                assert_eq!(result.count, count);
                assert_eq!(fs::read(&result.path).unwrap(), expected);
                fs::remove_file(result.path).unwrap();
            }
        }
    }
}

#[test]
fn large_fields_are_chunked_and_cancel_without_publishing_partial_artifacts() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("export.jsonl");
    let job = ExportJob::start(Some("cancel-large-field")).unwrap().unwrap();
    struct CancelAfterChunk {
        largest: usize,
    }
    impl Write for CancelAfterChunk {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.largest = self.largest.max(bytes.len());
            assert!(cancel_export("cancel-large-field"));
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut sink = CancelAfterChunk { largest: 0 };
    let error = atomic_durable_write_with(&target, |_| {
        let mut writer =
            CountedWriter { writer: &mut sink, bytes: 0, checked_at: 0, job: Some(&job) };
        writer.write_all(&vec![b'x'; 2 * WRITE_CHECK_BYTES as usize])?;
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("export-cancelled"));
    assert_eq!(sink.largest, WRITE_CHECK_BYTES as usize);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    job.complete::<()>(&Err(ExportCancelled.into()));
}

#[test]
fn ordinary_walk_consumes_rows_immediately_and_keeps_only_fixed_write_buffers() {
    for key in [None, Some([76; 32])] {
        let vault = seeded_archive(100_000, key);
        let mut buffer = BufWriter::with_capacity(EXPORT_BUFFER_BYTES, io::sink());
        let mut visited = 0usize;
        let format = ExportFormat::Jsonl;
        let mut writer = ExportWriter::new(&mut buffer, &format).unwrap();
        walk_history_for_export(&vault, HistoryQuery::default(), None, |row| {
            writer.item(row)?;
            visited += 1;
            assert!(writer.writer.buffer().len() <= EXPORT_BUFFER_BYTES);
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, 100_000);
        assert_eq!(writer.finish().unwrap(), visited);
        assert_eq!(buffer.capacity(), EXPORT_BUFFER_BYTES);
    }
}

#[test]
fn absent_cached_totals_do_not_prevent_a_tracked_export() {
    for key in [None, Some([76; 32])] {
        let vault = seeded_archive(4, key);
        vault
            .store()
            .write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
                s.execute("DELETE FROM archive_totals WHERE name='visits'", ())?;
                Ok(())
            })
            .unwrap();
        let result = export_history(
            &vault,
            request("missing-totals", HistoryQuery::default(), ExportFormat::Jsonl),
        )
        .unwrap();
        assert_eq!(result.count, 4);
        let progress = get_export_progress("missing-totals").unwrap();
        assert_eq!(progress.state, ExportState::Done);
        assert_eq!(progress.total_rows, None);
    }
}

#[test]
fn exports_start_at_the_first_match_regardless_of_screen_pagination() {
    for key in [None, Some([76; 32])] {
        let vault = seeded_archive(1205, key);
        let result = export_history(
            &vault,
            ExportRequest {
                export_id: None,
                query: HistoryQuery {
                    page: Some(9),
                    cursor: Some("1700000000100|400".into()),
                    limit: Some(1),
                    ..Default::default()
                },
                format: ExportFormat::Jsonl,
            },
        )
        .unwrap();
        assert_eq!(result.count, 1205);
    }
}
