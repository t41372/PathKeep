//! Contracts for physical coordinates and bounded evidence reads. All expected offsets
//! come from literal input bytes, independently of serde and the archive's framer.
use agent_history::read_model::*;
use agent_history::{Archive, Artifact, ArtifactKind, Harness, ParseContext, SourceRoot};
use sha2::{Digest, Sha256};
use std::io::{BufReader, Read};

fn locations(
    bytes: &[u8],
    harness: Harness,
    transport: Transport,
    limit: usize,
) -> agent_history::Result<Vec<RecordLocator>> {
    let mut found = Vec::new();
    scan_records(
        &mut BufReader::with_capacity(1, bytes),
        harness,
        transport,
        &ParseContext { max_record_bytes: limit, ..Default::default() },
        &JobControl::default(),
        &mut |record, _, _| {
            found.push(record);
            Ok(())
        },
    )?;
    Ok(found)
}

#[test]
fn physical_lines_include_bom_crlf_blank_oversize_and_split_scalar() {
    let first = "\u{feff}{\"s\":\"🦀\"}\r\n";
    let bytes = format!("{first}\n{}\n{{}}", "x".repeat(80));
    let rows = locations(bytes.as_bytes(), Harness::Pi, Transport::Plain, 24).unwrap();
    let positions: Vec<_> = rows.iter().map(|r| (r.ordinal.0, r.start.0, r.length)).collect();
    assert_eq!(positions, [(0, 0, 17), (1, 17, 1), (2, 18, 81), (3, 99, 2)]);
    assert_eq!(
        rows.iter().map(|row| row.coverage).collect::<Vec<_>>(),
        [
            RecordCoverage::Object,
            RecordCoverage::Blank,
            RecordCoverage::Oversized,
            RecordCoverage::Object
        ]
    );
}

#[test]
fn arrays_and_envelopes_locate_split_elements_without_separators() {
    let item = r#"{"s":"嵌套],\""}"#;
    for (tool, prefix, suffix) in [
        (Harness::Continue, "{\"unused\":[{}],\"history\":[ ", " ]}"),
        (Harness::Cline, "[ ", " ]"),
        (Harness::RooCode, "[ ", " ]"),
    ] {
        let input = format!("{prefix}{item}, null, {{}}{suffix}");
        let rows = locations(input.as_bytes(), tool, Transport::Plain, 64).unwrap();
        assert_eq!(rows.len(), 3);
        for (record, expected) in rows.iter().zip([item, "null", "{}"]) {
            let at = record.start.0 as usize;
            assert_eq!(&input[at..at + record.length as usize], expected);
        }
    }
}

#[test]
fn native_frame_coordinates_survive_records_crossing_frames() {
    let slices: [&[u8]; 3] = [b"{\"a\":", b"2}\n", b"{}\n"];
    let frames: Vec<_> = slices.iter().map(|s| zstd::stream::encode_all(*s, 1).unwrap()).collect();
    let rows = locations(&frames.concat(), Harness::Codex, Transport::Zstd, 32).unwrap();
    assert_eq!(rows[0].length, 8);
    let restart = rows[1].frame.unwrap();
    assert_eq!(
        (restart.native.0, restart.decoded.0, rows[1].start.0),
        ((frames[0].len() + frames[1].len()) as u64, 8, 8)
    );
    assert_eq!(
        locations(&[], Harness::Codex, Transport::Zstd, 32).unwrap_err().code(),
        "invalid_native_transport"
    );
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(temp.path()).unwrap();
    let info = archive.revision(retain(&archive, &frames.concat())).unwrap();
    for (row, expected) in rows.iter().zip([b"{\"a\":2}\n".as_slice(), b"{}\n"]) {
        assert_eq!(
            archive.record_bytes(&info, row, 0, 32, &Default::default()).unwrap().bytes,
            expected
        );
    }
    let mut damaged = frames.concat();
    damaged.pop();
    assert_eq!(
        locations(&damaged, Harness::Codex, Transport::Zstd, 32).unwrap_err().code(),
        "invalid_native_transport"
    );
}

fn retain(archive: &Archive, data: &[u8]) -> i64 {
    retain_for(archive, data, Harness::Pi)
}

fn retain_for(archive: &Archive, data: &[u8], harness: Harness) -> i64 {
    let root = SourceRoot { harness, root: "/fixture".into() };
    let artifact = Artifact {
        path: "/fixture/a.jsonl".into(),
        relative_path: "a.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let rev = archive.begin_revision("fixture", &root, &artifact).unwrap();
    for (ordinal, chunk) in data.chunks(5).enumerate() {
        archive.append_chunk(rev, ordinal as u64, chunk.to_vec()).unwrap();
    }
    archive
        .finish_revision(rev, &hex::encode(Sha256::digest(data)), data.len() as u64)
        .unwrap()
        .revision_id
}

#[test]
fn retained_window_crosses_three_slices_and_clips_shared_prefix() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    let bytes = b"{}\n{\"text\":\"abcd\"}";
    let old = retain(&archive, bytes);
    retain(&archive, &[bytes.as_slice(), b"\n{}\n"].concat());
    let mut shared = Vec::new();
    archive.raw_reader(old).unwrap().take(bytes.len() as u64 + 4).read_to_end(&mut shared).unwrap();
    assert_eq!(shared, bytes, "the old revision ends inside the successor slice");
    let rows = locations(bytes, Harness::Pi, Transport::Plain, 32).unwrap();
    let range = &rows[1];
    let page = archive
        .record_bytes(&archive.revision(old).unwrap(), range, 0, 64, &JobControl::default())
        .unwrap();
    assert_eq!(page.bytes, &bytes[3..]);
    assert_eq!(page.next, None);
    for at in [1, 2, 7, 12] {
        let page = archive
            .record_bytes(&archive.revision(old).unwrap(), range, at, 3, &JobControl::default())
            .unwrap();
        let start = 3 + at as usize;
        assert_eq!(page.bytes, &bytes[start..(start + 3).min(bytes.len())]);
        assert_eq!(page.next, (start + 3 < bytes.len()).then_some(at + 3));
    }
    let mut beyond = range.clone();
    beyond.length += 1;
    assert!(
        archive
            .record_bytes(&archive.revision(old).unwrap(), &beyond, 0, 64, &JobControl::default())
            .is_err()
    );
}

#[test]
fn single_frame_replay_reports_progress_and_obeys_cancellation() {
    let bytes = [vec![b'x'; 262144], b"\n{}\n".to_vec()].concat();
    let native = zstd::stream::encode_all(bytes.as_slice(), 1).unwrap();
    let rows = locations(&native, Harness::Pi, Transport::Zstd, 32).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    let rev = retain(&archive, &native);
    let mut job = JobControl::default();
    let token = job.cancel.clone();
    job.progress = Some(std::sync::Arc::new(move |p| {
        if p.done > 0 {
            token.cancel()
        }
    }));
    assert_eq!(
        archive
            .record_bytes(&archive.revision(rev).unwrap(), &rows[1], 0, 32, &job)
            .unwrap_err()
            .code(),
        "cancelled"
    );
    assert_eq!(
        archive
            .record_bytes(&archive.revision(rev).unwrap(), &rows[1], 0, 32, &Default::default())
            .unwrap()
            .bytes,
        b"{}\n"
    );
}

#[test]
fn legacy_pi_parent_and_unknown_deepseek_version_come_from_the_header() {
    for (harness, bytes, expected) in [
        (Harness::Pi, "{\"type\":\"session\",\"id\":\"p\"}\n{}\n{}", Some("legacy:1")),
        (
            Harness::Deepseek,
            "{\"type\":\"session\",\"id\":\"d\",\"version\":99,\"createdAt\":0}\n{\"type\":\"user/message\",\"data\":{\"content\":[{\"type\":\"text\",\"text\":\"hidden\"}]}}",
            None,
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let archive = Archive::create_unencrypted(temp.path()).unwrap();
        let revision = retain_for(&archive, bytes.as_bytes(), harness);
        let mut last = None;
        archive
            .scan_revision(
                revision,
                &Default::default(),
                &Default::default(),
                &mut |record, ctx, _| {
                    last = Some((record, ctx.clone()));
                    Ok(())
                },
            )
            .unwrap();
        archive
            .read_window(
                &archive.revision(revision).unwrap(),
                &[last.unwrap()],
                1024,
                &Default::default(),
                &mut |row| {
                    let event = row.interpretation.unwrap().event;
                    assert_eq!(event.parent_id.as_deref(), expected);
                    assert_eq!(event.text, "");
                    Ok(())
                },
            )
            .unwrap();
    }
}

#[test]
fn claude_window_keeps_thinking_and_call_input_separate_from_public_text() {
    let raw = br#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"why"},{"type":"tool_use","name":"read","input":{"path":"a"}},{"type":"text","text":"done"}]}}"#;
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    let revision = retain(&archive, raw);
    let mut outline = Vec::new();
    scan_records(
        &mut raw.as_slice(),
        Harness::ClaudeCode,
        Transport::Plain,
        &Default::default(),
        &Default::default(),
        &mut |record, ctx, _| {
            outline.push((record, ctx.clone()));
            Ok(())
        },
    )
    .unwrap();
    archive
        .read_window(
            &archive.revision(revision).unwrap(),
            &outline,
            1024,
            &Default::default(),
            &mut |row| {
                let interpreted = row.interpretation.unwrap();
                assert_eq!(interpreted.event.text, "why\ndone");
                assert_eq!(
                    interpreted.parts.iter().map(|part| part.kind).collect::<Vec<_>>(),
                    [PartKind::Thinking, PartKind::CallInput, PartKind::Text]
                );
                assert!(interpreted.parts[1].text.contains("path"));
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn empty_index_is_distinct_from_a_committed_zero_watermark() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    assert_eq!(archive.index_lag().unwrap(), IndexLag { indexed_through: None, target: 0 });
    archive.synchronize_search().unwrap();
    assert_eq!(archive.index_lag().unwrap(), IndexLag { indexed_through: Some(0), target: 0 });
}

#[test]
fn selection_skips_a_large_plain_gap_and_checks_revision_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    let bytes = [b"{}\n".as_slice(), &vec![b' '; 1024], b"\n{}\n"].concat();
    let rev = retain(&archive, &bytes);
    let mut info = archive.revision(rev).unwrap();
    let mut selected = Vec::new();
    scan_records(
        &mut bytes.as_slice(),
        Harness::Pi,
        Transport::Plain,
        &Default::default(),
        &Default::default(),
        &mut |record, ctx, value| {
            if value.is_some() {
                selected.push((record, ctx.clone()))
            }
            Ok(())
        },
    )
    .unwrap();
    let job = JobControl { max_decoded_bytes: 12, ..Default::default() };
    let mut seen = Vec::new();
    archive
        .read_window(&info, &selected, 8, &job, &mut |row| {
            seen.push(row.record.ordinal.0);
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, [0, 2]);
    info.digest = "different revision".into();
    assert_eq!(
        archive.record_bytes(&info, &selected[0].0, 0, 7, &job).unwrap_err().code(),
        "stale_cursor"
    );
}

#[test]
fn opaque_fields_are_raw_only_even_when_named_content() {
    for tool in [Harness::Pi, Harness::ClaudeCode, Harness::Deepseek, Harness::Codex] {
        let raw=br#"{"type":"unknown-future","content":"opaque","payload":{"type":"unknown-future","content":"opaque"},"data":{"content":"opaque"}}"#;
        let mut context = None;
        scan_records(
            &mut raw.as_slice(),
            tool,
            Transport::Plain,
            &Default::default(),
            &Default::default(),
            &mut |r, c, v| {
                context = Some((r, c.clone(), v.unwrap().clone()));
                Ok(())
            },
        )
        .unwrap();
        let (locator, ctx, value) = context.unwrap();
        let interpreted = ctx.interpret(locator.ordinal, value).unwrap();
        assert!(interpreted.parts.is_empty(), "{tool}");
    }
}

#[test]
fn maintenance_cancels_a_reader_waiting_in_its_sink() {
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    let rev = retain(&archive, b"{}\n");
    let job = JobControl::default();
    let (ready, entered) = mpsc::sync_channel(0);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            archive.scan_revision(rev, &Default::default(), &job, &mut |_, _, _| {
                ready.send(()).unwrap();
                let deadline = Instant::now() + Duration::from_secs(2);
                while job.cancel.check().is_ok() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(job.cancel.check().is_err(), "maintenance left the reader token active");
                Err(agent_history::Error::Cancelled)
            })
        });
        entered.recv().unwrap();
        let _pause = archive.pause_jobs();
        assert_eq!(reader.join().unwrap().unwrap_err().code(), "cancelled");
    });
}

#[test]
fn scan_coverage_counts_physical_rows_and_bounds_failure_samples() {
    let bytes = [b"{}\n".as_slice(), &b"?\n".repeat(136), b"null\n"].concat();
    let report = scan_records(
        &mut bytes.as_slice(),
        Harness::Codex,
        Transport::Plain,
        &Default::default(),
        &Default::default(),
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    assert_eq!((report.physical_records, report.objects), (138, 1));
    let Coverage::Partial { reasons } = report.coverage else {
        panic!("damaged lines were declared complete")
    };
    assert_eq!(reasons.len(), 129);
    assert_eq!(reasons.last().unwrap().detail.as_deref(), Some("9"));
}

#[test]
fn progress_reports_phase_units_and_completed_work() {
    use std::sync::{Arc, Mutex};
    let pulses = Arc::new(Mutex::new(Vec::new()));
    let captured = pulses.clone();
    let job = JobControl {
        progress: Some(Arc::new(move |p| {
            captured.lock().unwrap().push((p.phase, p.unit, p.done, p.total))
        })),
        ..Default::default()
    };
    scan_records(
        &mut b"{}\n".as_slice(),
        Harness::Codex,
        Transport::Plain,
        &Default::default(),
        &job,
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    let pulses = pulses.lock().unwrap();
    assert_eq!(pulses.first(), Some(&(ReadPhase::Framing, ProgressUnit::DecodedBytes, 0, None)));
    assert_eq!(pulses.last().unwrap().2, 3);
}
