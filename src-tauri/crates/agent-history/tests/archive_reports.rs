//! Responsibilities: prove report durability and honest backup-run termination.
//! Not responsible for: generating source diagnostics or CLI presentation.
//! Failure modes, written before implementation: coverage disappears on reopen;
//! a later error overwrites a completed backup; abandoned runs stay running;
//! an oversized report partially mutates canonical state; a report names unretained bytes.
use agent_history::{
    archive::Archive,
    model::{Artifact, ArtifactKind, Diagnostic, Harness, ParseReport, Reference, SourceRoot},
};
use sha2::{Digest, Sha256};
use vault_crypto::KdfParams;

const PASSWORD: &[u8] = b"report fixture password";

fn archive() -> (tempfile::TempDir, Archive) {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &temp.path().join("archive"),
        PASSWORD,
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    (temp, archive)
}

fn revision(archive: &Archive) -> i64 {
    let source = SourceRoot { harness: Harness::Codex, root: "/synthetic/codex".into() };
    let artifact = Artifact {
        path: "/synthetic/codex/chat.jsonl".into(),
        relative_path: "chat.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let id = archive.begin_revision("source", &source, &artifact).unwrap();
    let bytes = b"{\"future_record\":true}\n";
    archive.append_chunk(id, 0, bytes.to_vec()).unwrap();
    archive
        .finish_revision(id, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id
}

fn report() -> ParseReport {
    ParseReport {
        event_count: 1,
        diagnostics: vec![Diagnostic {
            code: "unsupported_event".into(),
            path: Some("chat.jsonl".into()),
            ordinal: Some(0),
            detail: None,
        }],
        references: vec![Reference {
            kind: "lineage".into(),
            target: "parent-rollout".into(),
            end_byte_offset: Some(4096),
            end_ordinal_exclusive: Some(20),
        }],
        complete: false,
        ..ParseReport::default()
    }
}

#[test]
fn parse_coverage_and_native_references_survive_reopen() {
    let (temp, archive) = archive();
    let revision = revision(&archive);
    assert_eq!(archive.load_parse_report(revision).unwrap(), None);
    archive.save_parse_report(revision, &report()).unwrap();
    drop(archive);
    let reopened = Archive::open(&temp.path().join("archive"), PASSWORD).unwrap();
    assert_eq!(reopened.load_parse_report(revision).unwrap(), Some(report()));
    assert!(reopened.save_parse_report(i64::MAX, &report()).is_err());
}

#[test]
fn parser_versions_do_not_overwrite_each_others_interpretation_of_the_same_raw_bytes() {
    let (_temp, archive) = archive();
    let revision = revision(&archive);
    let original = report();
    let mut changed = original.clone();
    changed.event_count = 2;
    changed.complete = true;
    archive.save_versioned_parse_report(revision, "parser-v1", &original).unwrap();
    archive.save_versioned_parse_report(revision, "parser-v2", &changed).unwrap();
    assert_eq!(archive.load_versioned_parse_report(revision, "parser-v1").unwrap(), Some(original));
    assert_eq!(archive.load_versioned_parse_report(revision, "parser-v2").unwrap(), Some(changed));
    assert_eq!(archive.load_versioned_parse_report(revision, "parser-v3").unwrap(), None);
}

#[test]
fn retained_reference_bindings_survive_aliases_rewrites_and_missing_live_files() {
    let (temp, archive) = archive();
    let parent = revision(&archive);
    let source = SourceRoot { harness: Harness::Codex, root: "/synthetic/attachments".into() };
    let artifact = Artifact {
        path: "/synthetic/attachments/item.png".into(),
        relative_path: "item.png".into(),
        kind: ArtifactKind::Attachment,
        project_hint: None,
        generation: None,
        current: true,
    };
    let reference = Reference {
        kind: "attachment".into(),
        target: "/old-alias/item.png".into(),
        end_byte_offset: None,
        end_ordinal_exclusive: None,
    };
    let capture = |bytes: &[u8]| {
        let staged = archive.begin_revision("attachment", &source, &artifact).unwrap();
        archive.append_chunk(staged, 0, bytes.to_vec()).unwrap();
        archive
            .finish_revision(staged, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
            .unwrap()
            .revision_id
    };
    let old = capture(b"old image bytes");
    archive.bind_reference(parent, &reference, old).unwrap();
    archive.bind_reference(parent, &reference, old).unwrap();
    let changed = capture(b"changed image bytes");
    archive.bind_reference(parent, &reference, changed).unwrap();
    assert!(archive.bind_reference(parent, &reference, i64::MAX).is_err());
    assert_eq!(archive.bound_reference(parent, &reference).unwrap().unwrap().id, changed);
    let mut different_range = reference.clone();
    different_range.end_byte_offset = Some(7);
    assert!(archive.bound_reference(parent, &different_range).unwrap().is_none());
    drop(archive);
    let reopened = Archive::open(&temp.path().join("archive"), PASSWORD).unwrap();
    let bound = reopened.bound_reference(parent, &reference).unwrap().unwrap();
    assert_eq!(bound.source_id, "attachment");
    assert_eq!(bound.id, changed);
    assert!(reopened.raw_reader(old).is_ok());
}

#[test]
fn every_parser_accepted_reference_fits_the_durable_binding_boundary() {
    let (_temp, archive) = archive();
    let raw = revision(&archive);
    let mut reference = Reference {
        kind: "attachment".into(),
        target: "x".repeat(64 * 1024),
        end_byte_offset: None,
        end_ordinal_exclusive: None,
    };
    archive.bind_reference(raw, &reference, raw).unwrap();
    assert_eq!(archive.bound_reference(raw, &reference).unwrap().unwrap().id, raw);
    reference.target.push('x');
    assert!(archive.bind_reference(raw, &reference, raw).is_err());
    assert!(archive.bound_reference(raw, &reference).is_err());
    reference.target.pop();
    reference.kind = "k".repeat(257);
    assert!(archive.bind_reference(raw, &reference, raw).is_err());
}

#[test]
fn terminal_backup_state_and_report_cannot_be_overwritten() {
    let (temp, archive) = archive();
    let run = archive.start_backup_run().unwrap();
    assert_eq!(archive.backup_run(run).unwrap().status, "running");
    let complete = serde_json::json!({"status":"complete","captured":3});
    archive.end_backup_run(run, "complete", &complete).unwrap();
    assert!(archive.end_backup_run(run, "failed", &serde_json::json!({"late":"error"})).is_err());
    drop(archive);
    let reopened = Archive::open(&temp.path().join("archive"), PASSWORD).unwrap();
    let stored = reopened.backup_run(run).unwrap();
    assert_eq!(stored.status, "complete");
    assert_eq!(stored.report, Some(complete));
    assert!(stored.finished_at.is_some());
}

#[test]
fn reopen_marks_abandoned_runs_interrupted_in_multiple_bounded_batches() {
    let (temp, archive) = archive();
    let mut runs = Vec::new();
    for _ in 0..131 {
        runs.push(archive.start_backup_run().unwrap());
    }
    let ended = archive.start_backup_run().unwrap();
    archive.end_backup_run(ended, "partial", &serde_json::json!({"captured":1})).unwrap();
    drop(archive);
    let reopened = Archive::open(&temp.path().join("archive"), PASSWORD).unwrap();
    for run in runs {
        let stored = reopened.backup_run(run).unwrap();
        assert_eq!(stored.status, "interrupted");
        assert!(stored.finished_at.is_some());
        assert!(reopened.end_backup_run(run, "complete", &serde_json::json!({})).is_err());
    }
    assert_eq!(reopened.backup_run(ended).unwrap().status, "partial");
}

#[test]
fn invalid_or_oversized_reports_leave_existing_state_untouched() {
    let (_temp, archive) = archive();
    let revision = revision(&archive);
    archive.save_parse_report(revision, &report()).unwrap();
    let mut oversized = report();
    oversized.diagnostics[0].detail = Some("x".repeat(16 * 1024 * 1024));
    assert!(archive.save_parse_report(revision, &oversized).is_err());
    assert_eq!(archive.load_parse_report(revision).unwrap(), Some(report()));
    let run = archive.start_backup_run().unwrap();
    assert!(archive.end_backup_run(run, "running", &serde_json::json!({})).is_err());
    let huge = serde_json::json!({"detail":"x".repeat(16 * 1024 * 1024)});
    assert!(archive.end_backup_run(run, "complete", &huge).is_err());
    assert_eq!(archive.backup_run(run).unwrap().status, "running");
    archive.end_backup_run(run, "failed", &serde_json::json!({"reason":"fixture"})).unwrap();
    assert_eq!(archive.backup_run(run).unwrap().status, "failed");
}
