//! Responsibilities: prove parser upgrades use retained evidence, even after native deletion.
//! Not responsible for: native restore, mock storage, or making live reference paths authoritative.
//! Failure modes specified before implementation: force reparse skips unchanged bytes; a new
//! parser mutates the visible old generation before publication; larger budgets cannot recover
//! retained records; references trigger fresh filesystem reads; missing evidence looks complete.

use agent_history::{
    Archive, Harness, SourceRoot,
    capture::BackupOptions,
    query::{EventQuery, SessionQuery},
    search::SearchQuery,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use vault_crypto::KdfParams;

struct Fixture {
    _temporary: tempfile::TempDir,
    archive: Arc<Archive>,
    root: SourceRoot,
    transcript: PathBuf,
    evidence_name: &'static str,
}

impl Fixture {
    fn new(name: &'static str) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = SourceRoot { harness: Harness::Pi, root: temporary.path().join("native-pi") };
        let transcript = root.root.join("sessions/project/session.jsonl");
        fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        let archive = Arc::new(
            Archive::create_with_params(
                &temporary.path().join("archive"),
                b"reparse-synthetic-password",
                KdfParams { memory_kib: 8192, iterations: 1, lanes: 1 },
            )
            .unwrap(),
        );
        Self { _temporary: temporary, archive, root, transcript, evidence_name: name }
    }

    fn write(&self, messages: &[Value]) -> Vec<u8> {
        let mut rows = vec![json!({"type":"session","version":3,"id":"native-reparse-id",
            "timestamp":"2026-11-11T01:00:00.000Z","cwd":"/synthetic/reparse"})];
        rows.extend_from_slice(messages);
        let mut bytes = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut bytes, &row).unwrap();
            bytes.push(b'\n');
        }
        fs::write(&self.transcript, &bytes).unwrap();
        bytes
    }

    fn source_id(&self) -> String {
        self.archive.list_sessions(&SessionQuery::default()).unwrap().items[0].source_id.clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../artifacts/agent-history/reparse");
        let report = json!({
            "scenario":self.evidence_name,"seed":"retained-reparse-v1",
            "status":if std::thread::panicking(){"failed"}else{"complete"},
            "private_data":false,
        });
        let _ = fs::create_dir_all(&directory);
        let _ = fs::write(
            directory.join(format!("{}.json", self.evidence_name)),
            serde_json::to_vec_pretty(&report).unwrap(),
        );
    }
}

fn message(id: &str, text: String) -> Value {
    json!({"type":"message","id":id,"parentId":null,
        "timestamp":"2026-11-11T01:00:01.000Z",
        "message":{"role":"user","content":text,"timestamp":1794358801000_i64}})
}

fn search(text: &str) -> SearchQuery {
    SearchQuery { text: text.into(), harness: Some(Harness::Pi), project_path: None, limit: 10 }
}

#[test]
fn larger_parser_budget_recovers_deleted_source_without_new_raw_revision() {
    let fixture = Fixture::new("deleted-source-budget-upgrade");
    let expected = fixture.write(&[
        message("small", "retainedsmallneedle".into()),
        message("large", format!("{} recoverabletailneedle", "x".repeat(4096))),
    ]);
    let initial = BackupOptions { max_record_bytes: 512, ..BackupOptions::default() };
    let saved = fixture.archive.backup(std::slice::from_ref(&fixture.root), &initial).unwrap();
    assert_eq!(saved.status, "complete");
    let outcome = saved.capture_outcome.unwrap();
    assert_eq!(outcome.warning_count, 0);
    assert!(outcome.note_count > 0);
    let source_id = fixture.source_id();
    let before = fixture.archive.session(&conversation(&fixture.archive, &source_id)).unwrap();
    assert!(!before.complete);
    assert_eq!(fixture.archive.search(&search("recoverabletailneedle")).unwrap().total, 0);
    fs::remove_dir_all(&fixture.root.root).unwrap();

    let observed = Arc::new(Mutex::new(Vec::new()));
    let observer_state = observed.clone();
    let observer_archive = fixture.archive.clone();
    let observer_source = before.id.clone();
    let old_projection = before.projection_id;
    let options = BackupOptions {
        max_record_bytes: 64 * 1024,
        progress: Some(Arc::new(move |progress| {
            if progress.phase == "projection_staged" {
                let session = observer_archive.session(&observer_source).unwrap();
                let events = observer_archive.events(&EventQuery::new(&observer_source)).unwrap();
                observer_state.lock().unwrap().push(json!({
                    "old_generation_visible":session.projection_id==old_projection,
                    "large_event_visible":events.items.iter().any(|event|event.text.contains("recoverabletailneedle")),
                }));
            }
        })),
        ..BackupOptions::default()
    };
    let result = fixture.archive.reparse_source(&source_id, &options).unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.captured, 0);
    assert_eq!(result.unchanged, 1);
    assert_eq!(fixture.archive.revisions(&source_id, 100, None).unwrap().items.len(), 1);
    let after = fixture.archive.session(&conversation(&fixture.archive, &source_id)).unwrap();
    assert_eq!(after.revision_id, before.revision_id);
    assert_ne!(after.projection_id, before.projection_id);
    assert!(after.complete);
    assert_eq!(
        *observed.lock().unwrap(),
        vec![json!({"old_generation_visible":true,"large_event_visible":false})]
    );
    assert!(fixture.archive.search(&search("recoverabletailneedle")).unwrap().total > 0);
    let mut actual = Vec::new();
    fixture.archive.raw_reader(before.revision_id).unwrap().read_to_end(&mut actual).unwrap();
    assert_eq!(actual, expected);
    assert!(!fixture.root.root.exists());
}

#[test]
fn retained_attachment_coverage_survives_deleted_source_roots() {
    let fixture = Fixture::new("retained-attachment");
    let attachment = fixture.root.root.join("tool-output.txt");
    fs::write(&attachment, b"retained tool evidence").unwrap();
    fixture.write(&[json!({"type":"message","id":"tool","parentId":null,
        "timestamp":"2026-11-11T01:00:01.000Z",
        "message":{"role":"toolResult","toolCallId":"call","toolName":"bash",
            "content":[{"type":"text","text":"toolsummaryneedle"}],
            "details":{"fullOutputPath":attachment},"timestamp":1794358801000_i64}})]);
    let backup = fixture
        .archive
        .backup(std::slice::from_ref(&fixture.root), &BackupOptions::default())
        .unwrap();
    assert_eq!(backup.status, "complete", "initial capture report: {backup:?}");
    let source_id = fixture.source_id();
    fs::remove_dir_all(&fixture.root.root).unwrap();
    let result = fixture.archive.reparse_source(&source_id, &BackupOptions::default()).unwrap();
    assert_eq!(result.status, "complete", "retained reparse report: {result:?}");
    assert_eq!(result.captured, 0);
    assert!(fixture.archive.session(&conversation(&fixture.archive, &source_id)).unwrap().complete);
    assert!(!fixture.root.root.exists());
}

#[test]
fn missing_retained_attachment_is_partial_and_does_not_read_new_live_file() {
    let fixture = Fixture::new("missing-retained-attachment");
    let attachment = fixture.root.root.join("never-captured.txt");
    fixture.write(&[json!({"type":"message","id":"tool","parentId":null,
        "timestamp":"2026-11-11T01:00:01.000Z",
        "message":{"role":"toolResult","toolCallId":"call","toolName":"bash",
            "content":[{"type":"text","text":"nativetoolneedle"}],
            "details":{"fullOutputPath":attachment},"timestamp":1794358801000_i64}})]);
    fixture.archive.backup(std::slice::from_ref(&fixture.root), &BackupOptions::default()).unwrap();
    let source_id = fixture.source_id();
    fs::remove_dir_all(&fixture.root.root).unwrap();
    fs::create_dir_all(&fixture.root.root).unwrap();
    fs::write(&attachment, b"newlivefileneedlemustnotberead").unwrap();
    let result = fixture.archive.reparse_source(&source_id, &BackupOptions::default()).unwrap();
    assert_eq!(result.status, "partial");
    assert_eq!(result.captured, 0);
    assert!(result.issues.iter().any(|issue| issue.code.contains("attachment")));
    let view = fixture.archive.session(&conversation(&fixture.archive, &source_id)).unwrap();
    assert!(view.complete);
    assert_eq!(view.attachments_not_kept, 1);
    assert!(result.issues.iter().any(|issue| issue.code == "attachment_not_retained"
        && issue.detail.as_deref() == Some("missing")));
    assert_eq!(fixture.archive.search(&search("newlivefileneedlemustnotberead")).unwrap().total, 0);
    assert_eq!(fixture.archive.revisions(&source_id, 100, None).unwrap().items.len(), 1);
}

fn conversation(archive: &Archive, source: &str) -> String {
    archive
        .list_sessions(&agent_history::query::SessionQuery {
            put_away: "include".into(),
            ..Default::default()
        })
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.sources.iter().any(|member| member.source_id == source))
        .unwrap()
        .id
}
