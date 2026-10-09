//! Responsibilities: prove reader-based import emits batches before end of input.
//! Not responsible for: archive SQL or browser SQLite staging.
//! Failure modes: whole-file buffering, lost callback errors, trailing I/O errors
//! mistaken for successful completion.
use super::*;
use std::io::{self, Read};
struct BrokenTail;
impl Read for BrokenTail {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("broken tail"))
    }
}
#[derive(Default)]
struct Sink {
    batches: usize,
}
impl HistoryBatchConsumer for Sink {
    type Error = String;
    fn urls(&mut self, _: Vec<ParsedUrl>) -> Result<(), String> {
        Ok(())
    }
    fn visits(&mut self, rows: Vec<ParsedVisit>) -> Result<(), String> {
        assert!(rows.len() <= 10);
        self.batches += 1;
        Ok(())
    }
}
impl TakeoutSourceEvidenceConsumer<String> for Sink {
    fn source_evidence(&mut self, _: TakeoutSourceEvidenceChunk) -> Result<(), String> {
        Ok(())
    }
}
#[test]
fn json_reader_emits_batches_before_an_unreadable_tail() {
    let records=(0..21).map(|id|format!(r#"{{"url":"https://stream.test/{id}","title":"Page","time_usec":1711965600000000}}"#)).collect::<Vec<_>>().join(",");
    let input =
        std::io::Cursor::new(format!("{{\"Browser History\":[{records},")).chain(BrokenTail);
    let mut sink = Sink::default();
    let result = stream_payload_reader_with_sink(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        input,
        10,
        TakeoutStreamOptions {
            collect_source_evidence: true,
            retain_report_source_evidence: false,
        },
        &mut sink,
    );
    assert!(result.is_err());
    assert_eq!(sink.batches, 2, "JSON must not be buffered through the end before emitting work");
}

#[test]
fn json_reader_preserves_existing_trailing_byte_behavior() {
    let mut sink = Sink::default();
    let result = stream_payload_reader_with_sink(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        std::io::Cursor::new(b"{\"Browser History\":[]} trailing bytes"),
        10,
        TakeoutStreamOptions {
            collect_source_evidence: true,
            retain_report_source_evidence: false,
        },
        &mut sink,
    );
    assert!(result.is_ok());
}
