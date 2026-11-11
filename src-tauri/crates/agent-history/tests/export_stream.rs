//! Responsibilities: prove exported evidence survives real archive paging and output failures.
//! Not responsible for: choosing destination paths or rendering an export in a UI.
//!
//! Written before the exporter. Failure modes: raw export decompresses or reserializes bytes;
//! normalized export drops unknown payloads or records beyond its first page; Markdown content
//! closes its own fence; failed writes return success; a concurrently replaced projection mixes
//! revisions in one otherwise valid export; a reparse of unchanged raw bytes evades a raw-revision
//! guard or exposes staged records. Tests execute only in the final validation phase.

use agent_history::{
    Error,
    archive::Archive,
    export::ExportFormat,
    model::{Artifact, ArtifactKind, Harness, NativeEvent, SessionMetadata, SourceRoot},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use vault_crypto::KdfParams;

const SESSION: &str = "export-source";

fn archive() -> (tempfile::TempDir, Archive) {
    let directory = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &directory.path().join("archive"),
        b"export-test-password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    (directory, archive)
}

fn retain(archive: &Archive, bytes: &[u8]) -> i64 {
    retain_format(archive, bytes, "native")
}

fn retain_format(archive: &Archive, bytes: &[u8], format: &str) -> i64 {
    let source = SourceRoot { harness: Harness::Pi, root: "/fixture/agent".into() };
    let artifact = Artifact {
        path: source.root.join("sessions/native.jsonl.zst"),
        relative_path: "sessions/native.jsonl.zst".into(),
        kind: if format == "native" { ArtifactKind::Transcript } else { ArtifactKind::Sqlite },
        project_hint: None,
        generation: None,
        current: true,
    };
    let revision = archive.begin_revision_with_format(SESSION, &source, &artifact, format).unwrap();
    for (ordinal, bytes) in bytes.chunks(256 * 1024).enumerate() {
        archive.append_chunk(revision, ordinal as u64, bytes.to_vec()).unwrap();
    }
    archive
        .finish_revision(revision, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id
}

fn event(ordinal: u64, text: String) -> NativeEvent {
    NativeEvent {
        ordinal,
        native_id: Some(format!("event-{ordinal}")),
        parent_id: ordinal.checked_sub(1).map(|previous| format!("event-{previous}")),
        kind: "message".into(),
        role: Some("assistant".into()),
        timestamp: Some(1_700_000_000_000 + ordinal as i64),
        text,
        payload: json!({"type":"message","future":{"retain":[ordinal,true,null]}}),
    }
}

fn publish(archive: &Archive, revision: i64, events: Vec<NativeEvent>, complete: bool) {
    archive.begin_projection(SESSION, revision).unwrap();
    for page in events.chunks(64) {
        archive.append_events(SESSION, revision, page.to_vec()).unwrap();
    }
    archive.publish_projection(SESSION, revision, &metadata(), complete).unwrap();
}

fn metadata() -> SessionMetadata {
    SessionMetadata {
        native_id: "native-export-session".into(),
        project_path: Some("/fixture/project".into()),
        title: Some("<script>metadata must stay inert</script>".into()),
        created_at: Some(1_700_000_000_000),
        updated_at: Some(1_700_000_000_200),
        origin: "cli".into(),
        harness_version: None,
        parent_session_id: None,
        model: None,
    }
}

#[test]
fn raw_export_is_byte_exact_while_normalized_export_preserves_all_paged_evidence() {
    let (_directory, archive) = archive();
    let mut bytes = zstd::stream::encode_all(&b"raw generation one\n"[..], 1).unwrap();
    bytes.extend(zstd::stream::encode_all(&b"raw generation two\n"[..], 1).unwrap());
    let revision = retain(&archive, &bytes);
    let events: Vec<_> = (0..137).map(|n| event(n, format!("message {n}"))).collect();
    publish(&archive, revision, events.clone(), false);

    let mut raw = Vec::new();
    let raw_report = archive.write_raw(revision, &mut raw).unwrap();
    assert_eq!(raw, bytes);
    assert_eq!(raw_report.byte_len, bytes.len() as u64);
    assert_eq!(raw_report.revision_id, revision);
    assert_eq!(raw_report.source_capture_format, "native");
    assert_eq!(raw_report.projection_id, None);
    let raw_report_json = serde_json::to_value(&raw_report).unwrap();
    assert_eq!(raw_report_json["revision_id"], revision.to_string());
    assert!(raw_report_json["projection_id"].is_null());

    let mut normalized = Vec::new();
    let report = archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut normalized).unwrap();
    assert_eq!(report.event_count, 137);
    assert_eq!(report.byte_len, normalized.len() as u64);
    assert!(!report.complete);
    assert!(!report.native_resume_ready);
    assert_eq!(report.projection_id, archive.current_projection_id(SESSION).unwrap());
    assert_eq!(
        serde_json::to_value(&report).unwrap()["projection_id"],
        report.projection_id.unwrap().to_string()
    );
    let rows: Vec<Value> = normalized
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 138);
    assert_eq!(rows[0]["type"], "pathkeep_agent_history_export");
    assert_eq!(rows[0]["session"]["revision_id"], revision.to_string());
    assert_eq!(rows[0]["native_resume_ready"], false);
    assert_eq!(rows[0]["complete"], false);
    assert_eq!(rows[0]["source_capture_format"], "native");
    for (actual, expected) in rows[1..].iter().zip(events) {
        assert_eq!(actual, &serde_json::to_value(expected).unwrap());
    }
}

#[test]
fn logical_database_exports_identify_the_retained_format_without_claiming_native_bytes() {
    let (_directory, archive) = archive();
    let bytes = b"{\"type\":\"sqlite_row\",\"table\":\"threads\",\"values\":[\"fixture\"]}\n";
    let revision = retain_format(&archive, bytes, "pathkeep-sqlite-evidence-v1");
    publish(&archive, revision, vec![event(0, "snapshot projection".into())], true);

    let mut raw = Vec::new();
    let raw_report = archive.write_raw(revision, &mut raw).unwrap();
    assert_eq!(raw, bytes);
    assert_eq!(raw_report.source_capture_format, "pathkeep-sqlite-evidence-v1");
    assert!(!raw_report.native_resume_ready);

    let mut output = Vec::new();
    let report = archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut output).unwrap();
    assert_eq!(report.source_capture_format, "pathkeep-sqlite-evidence-v1");
    let header: Value =
        serde_json::from_slice(output.split(|byte| *byte == b'\n').next().unwrap()).unwrap();
    assert_eq!(header["source_capture_format"], "pathkeep-sqlite-evidence-v1");
    assert_eq!(header["native_resume_ready"], false);
}

#[test]
fn markdown_cannot_promote_native_content_to_active_markup() {
    let (_directory, archive) = archive();
    let revision = retain(&archive, b"markdown fixture");
    let hostile = "before\n```\n````````\n<script>alert(1)</script>\n# forged title\nafter";
    publish(&archive, revision, vec![event(0, hostile.into())], true);
    let mut output = Vec::new();
    let report = archive.write_normalized(SESSION, ExportFormat::Markdown, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert_eq!(report.event_count, 1);
    assert!(output.contains(hostile));
    let before = output.split(hostile).next().unwrap();
    let opener = before.lines().last().unwrap();
    let fence = opener.trim_end_matches("text");
    assert!(fence.len() > 8 && fence.bytes().all(|byte| byte == b'`'));
    let after = output.split_once(hostile).unwrap().1;
    assert!(after.starts_with(&format!("\n{fence}\n")));
    assert!(output.contains("\"native_resume_ready\": false"));
}

struct BrokenOutput;

impl Write for BrokenOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::StorageFull, "simulated full destination"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_failure_never_returns_a_successful_export_report() {
    let (_directory, archive) = archive();
    let revision = retain(&archive, b"output failure fixture");
    publish(&archive, revision, vec![event(0, "keep this".into())], true);
    for result in [
        archive.write_raw(revision, &mut BrokenOutput),
        archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut BrokenOutput),
        archive.write_normalized(SESSION, ExportFormat::Markdown, &mut BrokenOutput),
    ] {
        assert!(
            matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::StorageFull)
        );
    }
}

struct ReplaceOnWrite<'a> {
    archive: &'a Archive,
    replacement: Option<i64>,
}

impl Write for ReplaceOnWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(revision) = self.replacement.take() {
            publish(self.archive, revision, vec![event(0, "replacement".into())], true);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn replacing_a_projection_during_export_cannot_mix_two_revisions() {
    let (_directory, archive) = archive();
    let old = retain(&archive, b"old source version");
    publish(&archive, old, vec![event(0, "original".into())], true);
    let replacement = retain(&archive, b"new source version");
    let mut output = ReplaceOnWrite { archive: &archive, replacement: Some(replacement) };
    assert!(archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut output).is_err());
}

struct ReparseOnWrite<'a> {
    archive: &'a Archive,
    revision: Option<i64>,
}

impl Write for ReparseOnWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(revision) = self.revision.take() {
            let attempt = self
                .archive
                .prepare_projection(SESSION, revision, "fixture-parser-v2", true)
                .unwrap()
                .unwrap();
            self.archive
                .append_projection_events(&attempt, vec![event(0, "reparsed event".into())])
                .unwrap();
            self.archive.publish_projection_attempt(&attempt, &metadata(), true).unwrap();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn republishing_identical_raw_bytes_during_export_rejects_the_changed_projection() {
    let (_directory, archive) = archive();
    let revision = retain(&archive, b"unchanged raw evidence");
    publish(&archive, revision, vec![event(0, "original interpretation".into())], true);
    let original_projection = archive.current_projection_id(SESSION).unwrap();
    let mut output = ReparseOnWrite { archive: &archive, revision: Some(revision) };
    assert!(matches!(
        archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut output),
        Err(Error::StaleCursor)
    ));
    assert_eq!(archive.current_projection(SESSION).unwrap(), Some(revision));
    assert_ne!(archive.current_projection_id(SESSION).unwrap(), original_projection);
}

#[test]
fn staged_reparse_keeps_the_published_export_available_until_publication() {
    let (_directory, archive) = archive();
    let revision = retain(&archive, b"raw source stays unchanged");
    publish(&archive, revision, vec![event(0, "published interpretation".into())], true);
    let old_projection = archive.current_projection_id(SESSION).unwrap();
    let attempt =
        archive.prepare_projection(SESSION, revision, "fixture-parser-v2", true).unwrap().unwrap();
    archive
        .append_projection_events(&attempt, vec![event(0, "unpublished interpretation".into())])
        .unwrap();
    let mut output = Vec::new();
    let report = archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut output).unwrap();
    assert_eq!(report.projection_id, old_projection);
    let rows: Vec<Value> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(rows[1]["text"], "published interpretation");
    assert_eq!(rows[0]["session"]["projection_id"], old_projection.unwrap().to_string());
    archive.publish_projection_attempt(&attempt, &metadata(), true).unwrap();
    let mut replacement = Vec::new();
    let replacement_report =
        archive.write_normalized(SESSION, ExportFormat::Jsonl, &mut replacement).unwrap();
    assert_eq!(replacement_report.revision_id, report.revision_id);
    assert_ne!(replacement_report.projection_id, report.projection_id);
}
