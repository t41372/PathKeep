//! Failure modes: nested native/context JSON changes evidence identity when serializer features change.
//! Regression tests for the Takeout parser boundary.

use super::*;
use crate::types::{HistoryBatchConsumer, ParsedUrl, ParsedVisit};

fn browser_history_payload(records: &[&str]) -> String {
    format!(r#"{{"Browser History":[{}]}}"#, records.join(","))
}

/// A record with no time is skipped, never dated 1970; two records of one page are one URL
/// with two visits; and an error from the row sink comes back to the caller.
#[test]
fn browser_history_payload_skips_duplicates_and_error_paths() {
    let parsed = parse_payload(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        br#"{"Browser History":[{"url":"https://example.com/repeat","title":"Old","time_usec":"1711965600000000"},{"url":"https://example.com/repeat","title":"New","time_usec":1711969200000000},{"url":"https://example.com/missing-time"}]}"#,
    )
    .expect("browser history payload");
    assert_eq!(parsed.skipped_missing_visit_time, 1);
    assert_eq!(parsed.history.visits.len(), 2, "the record without a time is not a visit");
    assert!(
        parsed.history.visits.iter().all(|visit| visit.url != "https://example.com/missing-time"),
        "the record without a time gets no made-up date"
    );
    assert_eq!(parsed.history.urls.len(), 1);
    assert_eq!(parsed.history.urls[0].visit_count, 2);

    struct FailingConsumer;

    impl HistoryBatchConsumer for FailingConsumer {
        type Error = String;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Err("url sink offline".to_string())
        }

        fn visits(&mut self, _batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let sink_error = stream_payload(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
        1,
        &mut FailingConsumer,
    )
    .expect_err("canonical sink error");
    assert!(sink_error.to_string().contains("url sink offline"));
}

#[test]
fn stream_payload_with_options_can_skip_source_evidence_accumulation() {
    #[derive(Default)]
    struct CountingConsumer {
        visits: usize,
    }

    impl HistoryBatchConsumer for CountingConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits += batch.len();
            Ok(())
        }
    }

    let mut consumer = CountingConsumer::default();
    let report = stream_payload_with_options(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000,"client_id":"alpha"}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000,"client_id":"beta"}"#,
        ])
        .as_bytes(),
        10,
        TakeoutStreamOptions {
            collect_source_evidence: false,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .expect("stream payload without evidence");

    assert_eq!(consumer.visits, 2);
    assert!(report.history.typed_evidence.context.is_empty());
    assert!(report.history.native_entities.is_empty());
}

#[test]
fn stream_payload_with_sink_moves_source_evidence_out_of_the_report() {
    #[derive(Default)]
    struct RecordingSinkConsumer {
        visits: usize,
        source_chunks: usize,
        context_rows: usize,
        native_entities: usize,
    }

    impl HistoryBatchConsumer for RecordingSinkConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits += batch.len();
            Ok(())
        }
    }

    impl TakeoutSourceEvidenceConsumer<std::convert::Infallible> for RecordingSinkConsumer {
        fn source_evidence(
            &mut self,
            chunk: TakeoutSourceEvidenceChunk,
        ) -> Result<(), std::convert::Infallible> {
            self.source_chunks += 1;
            self.context_rows += chunk.typed_evidence.context.len();
            self.native_entities += chunk.native_entities.len();
            Ok(())
        }
    }

    let mut consumer = RecordingSinkConsumer::default();
    let report = stream_payload_with_sink(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000,"client_id":"alpha"}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000,"client_id":"beta"}"#,
        ])
        .as_bytes(),
        1,
        TakeoutStreamOptions {
            collect_source_evidence: true,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .expect("stream payload with source-evidence sink");

    assert_eq!(consumer.visits, 2);
    assert_eq!(consumer.context_rows, 2);
    assert_eq!(consumer.native_entities, 2);
    assert_eq!(consumer.source_chunks, 2);
    assert!(report.history.typed_evidence.context.is_empty());
    assert!(report.history.native_entities.is_empty());
}

#[test]
fn parse_payload_preserves_typed_url_and_session_payloads_as_native_entities() {
    let typed = parse_payload(
        "TypedUrl.json",
        KIND_TYPED_URL_JSON,
        br#"{"TypedUrl":[{"url":"https://example.com","title":"Example","hidden":false}]}"#,
    )
    .expect("parse typed url");
    let session = parse_payload(
        "Session.json",
        KIND_SESSION_JSON,
        br#"{"Session":[{"sessionTag":"device-1","tab":[{"navigation":[{"virtual_url":"https://example.com"}]}]}]}"#,
    )
    .expect("parse session");
    let fallback =
        parse_payload("TypedUrl.json", KIND_TYPED_URL_JSON, br#"{"TypedUrl":[{"other":true}]}"#)
            .expect("parse fallback native key");

    assert!(typed.history.visits.is_empty());
    assert_eq!(typed.history.native_entities.len(), 1);
    assert_eq!(typed.history.native_entities[0].native_primary_key, "https://example.com");
    assert_eq!(session.history.native_entities.len(), 1);
    assert_eq!(session.history.native_entities[0].native_primary_key, "device-1");
    assert_eq!(fallback.history.native_entities[0].native_primary_key, "row-0");
}

/// Native/context payload bytes are identity: changing object order must not duplicate evidence.
#[test]
fn native_payloads_keep_sorted_nested_json_across_serializer_features() {
    for (kind, input, expected) in [
        (
            KIND_BROWSER_JSON,
            r#"{"Browser History":[{"url":"https://example.com","time_usec":1711965600000000,"client_id":{"z":1,"a":2}}]}"#,
            r#"{"client_id":{"a":2,"z":1},"time_usec":1711965600000000,"url":"https://example.com"}"#,
        ),
        (
            KIND_MY_ACTIVITY_JSON,
            r#"[{"titleUrl":"https://example.com","time":"2025-04-22T18:30:56Z","products":["Chrome"],"client_id":{"z":1,"a":2}}]"#,
            r#"{"client_id":{"a":2,"z":1},"products":["Chrome"],"time":"2025-04-22T18:30:56Z","titleUrl":"https://example.com"}"#,
        ),
        (
            KIND_SESSION_JSON,
            r#"{"Session":[{"z":{"z":1,"a":2},"sessionTag":"device-1","a":[{"z":1,"a":2}]}]}"#,
            r#"{"a":[{"a":2,"z":1}],"sessionTag":"device-1","z":{"a":2,"z":1}}"#,
        ),
    ] {
        let parsed = parse_payload("fixture.json", kind, input.as_bytes()).unwrap();
        assert_eq!(parsed.history.native_entities[0].payload_json, expected);
        if kind == KIND_SESSION_JSON {
            assert!(parsed.history.typed_evidence.context.is_empty());
        } else {
            let context =
                parsed.history.typed_evidence.context.first().expect("browser context is retained");
            assert_eq!(context.value_json, r#"{"a":2,"z":1}"#);
        }
    }
}
