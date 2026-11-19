//! Exercises identity, resource admission and typed previews through the public evidence reader.
use agent_history::{
    Archive, Artifact, ArtifactKind, Harness, ParseContext, SourceRoot,
    archive::{RevisionInfo, SLICE_BYTES},
    read_model::*,
};
use sha2::{Digest, Sha256};
use std::{sync::mpsc, time::Duration};

fn capture(a: &Archive, path: &str, harness: Harness, bytes: &[u8]) -> RevisionInfo {
    let root = SourceRoot { harness, root: "/fixture".into() };
    let artifact = Artifact {
        kind: ArtifactKind::Transcript,
        path: root.root.join(path),
        relative_path: path.into(),
        current: true,
        generation: None,
        project_hint: None,
    };
    let id = a.begin_revision(path, &root, &artifact).unwrap();
    for (n, slice) in bytes.chunks(SLICE_BYTES).enumerate() {
        a.append_chunk(id, n as u64, slice.to_vec()).unwrap();
    }
    a.finish_revision(id, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64).unwrap();
    a.revision(id).unwrap()
}
fn outline(a: &Archive, info: &RevisionInfo) -> Vec<(RecordLocator, RecordContext)> {
    let mut rows = Vec::new();
    a.scan_revision(
        info.id,
        &ParseContext { max_record_bytes: 64 << 20, ..Default::default() },
        &Default::default(),
        &mut |r, c, _| {
            rows.push((r, c.clone()));
            Ok(())
        },
    )
    .unwrap();
    rows
}

#[test]
fn foreign_revision_rejects_both_reads_before_delivery() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let old = capture(&a, "a.jsonl", Harness::Pi, b"{\"id\":1}\n");
    let records = outline(&a, &old);
    for (path, bytes) in [("a.jsonl", b"{\"id\":2}\n"), ("b.jsonl", b"{\"id\":1}\n")] {
        let other = capture(&a, path, Harness::Pi, bytes);
        let mut seen = 0;
        let raw = a.record_bytes(&other, &records[0].0, 0, 32, &Default::default());
        let window = a.read_window(&other, &records, 32, &Default::default(), &mut |_| {
            seen += 1;
            Ok(())
        });
        assert_eq!(
            (raw.err().map(|e| e.code()), window.err().map(|e| e.code()), seen),
            (Some("stale_cursor"), Some("stale_cursor"), 0)
        );
    }
}

#[test]
fn large_tool_result_has_bounded_typed_preview_and_exact_raw() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    for size in [8 << 20, 17 << 20] {
        let text = "🦀".repeat(size / 4);
        let raw = serde_json::to_vec(
            &serde_json::json!({"type":"message","message":{"role":"toolResult","content":text}}),
        )
        .unwrap();
        let info = capture(&a, "a.jsonl", Harness::Pi, &raw);
        let records = outline(&a, &info);
        a.read_window(&info, &records, 4095, &Default::default(), &mut |r| {
            assert_eq!(r.raw.bytes, &raw[..4095]);
            assert_eq!(r.raw.next, Some(4095));
            let serialized = serde_json::to_value(&r).unwrap();
            assert_eq!(serialized["preview"]["parts"][0]["kind"], "CallResult");
            assert_eq!(serialized["preview"]["parts"][0]["text"].as_str(), Some(&text[..4092]));
            assert_eq!(serialized["preview"]["truncated"], true);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn cancelled_buffered_scan_stops_at_delivery() {
    let job = JobControl::default();
    let mut count = 0;
    let result = scan_records(
        &mut b"{}\n{}\n{}\n".as_slice(),
        Harness::Pi,
        Transport::Plain,
        &Default::default(),
        &job,
        &mut |_, _, _| {
            count += 1;
            job.cancel.cancel();
            Ok(())
        },
    );
    assert_eq!((count, result.unwrap_err().code()), (1, "cancelled"));
}
#[test]
fn queued_decoder_can_cancel_while_two_workers_are_busy() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let info = capture(&a, "a.jsonl", Harness::Pi, b"{}\n");
    let rows = outline(&a, &info);
    std::thread::scope(|s| {
        let (entered, ready) = mpsc::channel();
        let mut release = Vec::new();
        for _ in 0..2 {
            let (send, recv) = mpsc::channel();
            release.push(send);
            let entered = entered.clone();
            let a = &a;
            let info = &info;
            let rows = &rows;
            s.spawn(move || {
                a.read_window(info, rows, 32, &Default::default(), &mut |_| {
                    entered.send(()).unwrap();
                    recv.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(())
                })
                .unwrap()
            });
        }
        for _ in 0..2 {
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        let job = JobControl::default();
        let token = job.cancel.clone();
        let (done, answer) = mpsc::channel();
        let a = &a;
        let info = &info;
        s.spawn(move || {
            done.send(
                a.scan_revision(info.id, &Default::default(), &job, &mut |_, _, _| Ok(()))
                    .map(|_| ()),
            )
            .unwrap()
        });
        let early = answer.recv_timeout(Duration::from_millis(150));
        token.cancel();
        let outcome = early.ok().or_else(|| answer.recv_timeout(Duration::from_secs(1)).ok());
        for sender in release {
            sender.send(()).unwrap();
        }
        assert_eq!(outcome.unwrap().unwrap_err().code(), "cancelled");
    });
}

#[test]
fn empty_compressed_tail_needs_zero_decoded_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let raw = [vec![b' '; 1024 * 256], b"\n{}\n".to_vec()].concat();
    let native = zstd::stream::encode_all(raw.as_slice(), 1).unwrap();
    let info = capture(&a, "a.jsonl.zst", Harness::Pi, &native);
    let r = outline(&a, &info).pop().unwrap().0;
    let job = JobControl { max_decoded_bytes: 0, ..Default::default() };
    let page = a.record_bytes(&info, &r, r.length, 1, &job).unwrap();
    assert!(page.bytes.is_empty() && page.next.is_none());
}
#[test]
fn missing_array_slot_has_a_physical_rank_and_empty_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let info = capture(&a, "api_conversation_history.json", Harness::Cline, b"[{},,{}]");
    let mut records = Vec::new();
    let report = a
        .scan_revision(info.id, &Default::default(), &Default::default(), &mut |r, c, _| {
            records.push((r, c.clone()));
            Ok(())
        })
        .unwrap();
    assert_eq!((report.physical_records, report.objects), (3, 2));
    let r = &records[1].0;
    assert_eq!(
        (r.ordinal.0, r.start.0, r.length, r.coverage),
        (1, 4, 0, RecordCoverage::Malformed)
    );
    let mut seen = Vec::new();
    a.read_window(&info, &records, 32, &Default::default(), &mut |row| {
        seen.push((row.record.ordinal.0, row.raw.bytes));
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, [(0, b"{}".to_vec()), (1, vec![]), (2, b"{}".to_vec())]);
}
#[test]
fn pi_bash_parts_match_native_fields_and_keep_export_text() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let raw = br#"{"type":"message","message":{"role":"bashExecution","command":"pwd","output":"/work"}}"#;
    let info = capture(&a, "a.jsonl", Harness::Pi, raw);
    a.read_window(&info, &outline(&a, &info), 1024, &Default::default(), &mut |r| {
        let i = r.interpretation.unwrap();
        assert_eq!(i.event.text, "pwd\n/work");
        assert_eq!(
            i.parts.iter().map(|p| (p.kind, p.text.as_str())).collect::<Vec<_>>(),
            [(PartKind::CallInput, "pwd"), (PartKind::CallResult, "/work")]
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn changing_transport_on_an_archive_locator_is_rejected_before_decode() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let info = capture(&a, "a.jsonl", Harness::Pi, b"{}\n");
    let mut rows = outline(&a, &info);
    rows[0].0.frame = Some(FrameStart { native: NativeOffset(0), decoded: DecodedOffset(0) });
    assert_eq!(
        a.record_bytes(&info, &rows[0].0, 0, 3, &Default::default()).unwrap_err().code(),
        "stale_cursor"
    );
}

#[test]
fn typed_preview_keeps_large_native_ids_in_exact_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(tmp.path()).unwrap();
    let id = "large-id".repeat(4096);
    let raw = serde_json::to_vec(
        &serde_json::json!({"type":"message","id":id,"message":{"role":"user","content":"hello"}}),
    )
    .unwrap();
    let info = capture(&a, "a.jsonl", Harness::Pi, &raw);
    let entries = outline(&a, &info);
    a.read_window(&info, &entries, 64, &Default::default(), &mut |r| {
        let preview = r.preview.unwrap();
        assert!(preview.native_id.is_none(), "partial IDs must never become navigation anchors");
        assert!(preview.truncated);
        assert_eq!(preview.parts[0].text, "hello");
        Ok(())
    })
    .unwrap();
    let exact = a.record_bytes(&info, &entries[0].0, 0, raw.len(), &Default::default()).unwrap();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&exact.bytes).unwrap()["id"], id);
}
