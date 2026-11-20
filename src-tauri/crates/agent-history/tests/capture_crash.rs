//! Responsibilities: prove capture recovery through native files and process exit.
//! Not responsible for: mocking storage, adapter parsing, or native harness execution.
//!
//! Written before the backup pipeline. Failure modes: raw commit without projection
//! is never recovered; projection ahead of Tantivy returns a misleading empty result;
//! retries duplicate raw revisions or sessions; an already cancelled backup commits
//! new evidence. Real child processes exit at public progress boundaries without
//! dropping the encrypted archive. Every parent scenario retains synthetic evidence.

use agent_history::{
    Archive, Error, Harness, SourceRoot,
    capture::{BackupOptions, BackupProgress},
    query::{EventQuery, SessionQuery, SessionView},
    search::SearchQuery,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use vault_crypto::KdfParams;
use vault_store::CancelToken;

const PASSWORD: &[u8] = b"synthetic capture crash password";
const CHILD_ROOT: &str = "PATHKEEP_AGENT_CAPTURE_CHILD_ROOT";
const CHILD_SOURCE: &str = "PATHKEEP_AGENT_CAPTURE_CHILD_SOURCE";
const CHILD_PHASE: &str = "PATHKEEP_AGENT_CAPTURE_CHILD_PHASE";
const CHILD_CHECKPOINT: &str = "PATHKEEP_AGENT_CAPTURE_CHILD_CHECKPOINT";
const EXIT_AT_BOUNDARY: i32 = 93;

fn options() -> BackupOptions {
    BackupOptions {
        index_pause: Default::default(),
        max_record_bytes: 1024 * 1024,
        max_decoded_bytes: 16 * 1024 * 1024,
        cancel: CancelToken::new(),
        progress: None,
        approved_directories: None,
        read_git: true,
    }
}

fn query(text: &str) -> SearchQuery {
    SearchQuery { text: text.into(), harness: Some(Harness::Pi), project_path: None, limit: 10 }
}

fn fixture_rows() -> Vec<Value> {
    vec![
        json!({"type":"session","version":3,"id":"capture-crash-native-pi",
            "timestamp":"2026-11-11T00:00:00.000Z","cwd":"/synthetic/capture-project"}),
        json!({"type":"message","id":"root","parentId":null,
            "timestamp":"2026-11-11T00:00:01.000Z",
            "message":{"role":"user","content":"baselineword",
                "timestamp":1794355201000_i64}}),
        json!({"type":"message","id":"branch-a","parentId":"root",
            "timestamp":"2026-11-11T00:00:02.000Z",
            "message":{"role":"assistant","content":[{"type":"text","text":"branchaword"}],
                "timestamp":1794355202000_i64}}),
        json!({"type":"message","id":"branch-b","parentId":"root",
            "timestamp":"2026-11-11T00:00:03.000Z",
            "message":{"role":"user","content":"branchbword",
                "timestamp":1794355203000_i64}}),
    ]
}

fn appended_row() -> Value {
    json!({"type":"message","id":"resumed","parentId":"branch-b",
        "timestamp":"2026-11-11T00:00:04.000Z",
        "message":{"role":"user","content":"crashnewneedle",
            "timestamp":1794355204000_i64}})
}

fn jsonl(rows: &[Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for row in rows {
        serde_json::to_writer(&mut bytes, row).unwrap();
        bytes.push(b'\n');
    }
    bytes
}

struct Fixture {
    _temporary: tempfile::TempDir,
    archive_root: PathBuf,
    source: SourceRoot,
    transcript: PathBuf,
    checkpoint: PathBuf,
    old_bytes: Vec<u8>,
    new_bytes: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let archive_root = temporary.path().join("encrypted-archive");
        let source =
            SourceRoot { harness: Harness::Pi, root: temporary.path().join("synthetic-pi") };
        let transcript = source.root.join("agent/sessions/--synthetic-project--/chat.jsonl");
        let checkpoint = temporary.path().join("child-checkpoint.json");
        let mut rows = fixture_rows();
        let old_bytes = jsonl(&rows);
        rows.push(appended_row());
        let new_bytes = jsonl(&rows);
        fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        fs::write(&transcript, &old_bytes).unwrap();
        Self {
            _temporary: temporary,
            archive_root,
            source,
            transcript,
            checkpoint,
            old_bytes,
            new_bytes,
        }
    }

    fn baseline(&self) -> (Archive, SessionView) {
        let archive = Archive::create_with_params(
            &self.archive_root,
            PASSWORD,
            KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
        )
        .unwrap();
        archive.backup(std::slice::from_ref(&self.source), &options()).unwrap();
        archive.synchronize_search().unwrap();
        assert_eq!(archive.search(&query("baselineword")).unwrap().total, 1);
        let mut sessions = archive.list_sessions(&SessionQuery::default()).unwrap().items;
        assert_eq!(sessions.len(), 1);
        (archive, sessions.remove(0))
    }
}

struct Evidence {
    directory: PathBuf,
    checks: Vec<Value>,
}

impl Evidence {
    fn new(scenario: &str) -> Self {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../artifacts/agent-history/capture-crash")
            .join(scenario);
        fs::create_dir_all(&directory).unwrap();
        let fixture = json!({
            "schema_version":1,"scenario":scenario,"private_data":false,
            "native_format":"pi-jsonl-v3",
            "source_revision":"49fe0a237450eb003434d4020b0bfdf53bc6449c",
            "relative_path":"agent/sessions/--synthetic-project--/chat.jsonl",
            "baseline":fixture_rows(),"append":appended_row(),
            "reproduce":"cargo test --manifest-path src-tauri/Cargo.toml -p agent-history --test capture_crash",
        });
        fs::write(directory.join("fixture.json"), serde_json::to_vec_pretty(&fixture).unwrap())
            .unwrap();
        Self { directory, checks: vec![] }
    }

    fn equal(&mut self, name: &str, actual: Value, expected: Value) {
        self.checks.push(json!({"name":name,"expected":expected,"actual":actual}));
        assert_eq!(actual, expected, "{name}");
    }
}

impl Drop for Evidence {
    fn drop(&mut self) {
        let report = json!({
            "schema_version":1,"private_data":false,"checks":self.checks,
            "outcome":if std::thread::panicking() { "failed" } else { "passed" },
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
            let _ = fs::write(self.directory.join("report.json"), bytes);
        }
    }
}

fn raw_bytes(archive: &Archive, revision: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive.raw_reader(revision).unwrap().read_to_end(&mut bytes).unwrap();
    bytes
}

#[test]
fn child_exits_at_a_durable_capture_boundary() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else { return };
    let source = SourceRoot {
        harness: Harness::Pi,
        root: std::env::var_os(CHILD_SOURCE).expect("isolated child source").into(),
    };
    let phase = std::env::var(CHILD_PHASE).expect("requested child boundary");
    let checkpoint = PathBuf::from(std::env::var_os(CHILD_CHECKPOINT).unwrap());
    let archive = Archive::open(Path::new(&root), PASSWORD).unwrap();
    let mut settings = options();
    settings.progress = Some(Arc::new(move |progress: BackupProgress| {
        if progress.phase == phase {
            let evidence = json!({
                "phase":progress.phase,"source_id":progress.source_id,
                "revision_id":progress.revision_id,"bytes":progress.bytes,
            });
            fs::write(&checkpoint, serde_json::to_vec(&evidence).unwrap()).unwrap();
            // Windows flushes only through a handle that may write.
            fs::OpenOptions::new()
                .read(true)
                .write(cfg!(windows))
                .open(&checkpoint)
                .unwrap()
                .sync_all()
                .unwrap();
            // No destructor runs: this exercises the same durability boundary as
            // an abruptly lost owner process, without a timing-dependent kill.
            std::process::exit(EXIT_AT_BOUNDARY);
        }
    }));
    archive.backup(&[source], &settings).unwrap();
    panic!("the requested durable boundary was never emitted");
}

fn recover_at(phase: &str) {
    let mut evidence = Evidence::new(phase);
    let fixture = Fixture::new();
    let (archive, previous) = fixture.baseline();
    drop(archive);
    fs::write(&fixture.transcript, &fixture.new_bytes).unwrap();
    let source_modified = fs::metadata(&fixture.transcript).unwrap().modified().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_exits_at_a_durable_capture_boundary", "--nocapture"])
        .env(CHILD_ROOT, &fixture.archive_root)
        .env(CHILD_SOURCE, &fixture.source.root)
        .env(CHILD_PHASE, phase)
        .env(CHILD_CHECKPOINT, &fixture.checkpoint)
        .output()
        .unwrap();
    evidence.checks.push(json!({
        "name":"child process diagnostics",
        "stdout":String::from_utf8_lossy(&output.stdout),
        "stderr":String::from_utf8_lossy(&output.stderr),
    }));
    evidence.equal("child exited at requested boundary", json!(output.status.code()), json!(93));
    let checkpoint: Value =
        serde_json::from_slice(&fs::read(&fixture.checkpoint).unwrap()).unwrap();
    evidence.equal("observed phase", checkpoint["phase"].clone(), json!(phase));
    evidence.equal(
        "stable source identity",
        checkpoint["source_id"].clone(),
        json!(previous.source_id),
    );
    let retained_revision = checkpoint["revision_id"].as_i64().expect("committed revision id");

    let archive = Archive::open(&fixture.archive_root, PASSWORD).unwrap();
    evidence.equal(
        "both exact raw revisions survived process exit",
        json!(
            raw_bytes(&archive, previous.revision_id) == fixture.old_bytes
                && raw_bytes(&archive, retained_revision) == fixture.new_bytes
        ),
        json!(true),
    );
    let current = archive.session(&previous.id).unwrap();
    if phase == "raw_committed" {
        evidence.equal(
            "raw-only commit leaves the previous projection visible",
            json!(current.revision_id),
            json!(previous.revision_id),
        );
    } else {
        evidence.equal(
            "published projection survives before the index commit",
            json!(current.revision_id),
            json!(retained_revision),
        );
        evidence.equal(
            "search refuses a stale index after process exit",
            json!(matches!(archive.search(&query("crashnewneedle")), Err(Error::IndexBehind))),
            json!(true),
        );
    }

    for _ in 0..2 {
        archive.backup(std::slice::from_ref(&fixture.source), &options()).unwrap();
    }
    let sessions = archive.list_sessions(&SessionQuery::default()).unwrap();
    evidence.equal(
        "recovery and retries retain one session",
        json!(sessions.items.len()),
        json!(1),
    );
    evidence.equal(
        "retry uses the already durable raw revision",
        json!(sessions.items[0].revision_id),
        json!(retained_revision),
    );
    let revisions = archive.revisions(&previous.source_id, 100, None).unwrap();
    evidence.equal(
        "recovery and retries retain two raw versions",
        json!(revisions.items.len()),
        json!(2),
    );
    evidence.equal(
        "raw history fits this complete page",
        json!(revisions.next_cursor),
        Value::Null,
    );
    let events = archive.events(&EventQuery::new(previous.id)).unwrap();
    let event_ids =
        events.items.iter().filter_map(|event| event.native_id.as_deref()).collect::<Vec<_>>();
    evidence.equal(
        "both native branches and the resumed event survive recovery once",
        json!(
            ["root", "branch-a", "branch-b", "resumed"].iter().all(|wanted| event_ids
                .iter()
                .filter(|found| *found == wanted)
                .count()
                == 1)
        ),
        json!(true),
    );
    archive.synchronize_search().unwrap();
    evidence.equal(
        "new content is searchable after synchronization",
        json!(archive.search(&query("crashnewneedle")).unwrap().total),
        json!(1),
    );
    evidence.equal(
        "old branch remains searchable",
        json!(archive.search(&query("branchaword")).unwrap().total),
        json!(1),
    );
    evidence.equal(
        "source bytes and modification time remain unchanged",
        json!(
            fs::read(&fixture.transcript).unwrap() == fixture.new_bytes
                && fs::metadata(&fixture.transcript).unwrap().modified().unwrap()
                    == source_modified
        ),
        json!(true),
    );
}

#[test]
fn retry_after_raw_commit_recovers_projection_without_duplicating_evidence() {
    recover_at("raw_committed");
}

#[test]
fn retry_after_projection_publish_recovers_search_without_duplicating_sessions() {
    recover_at("projection_published");
}

#[test]
fn an_already_cancelled_capture_preserves_the_previous_revision_and_search() {
    let mut evidence = Evidence::new("already-cancelled");
    let fixture = Fixture::new();
    let (archive, previous) = fixture.baseline();
    fs::write(&fixture.transcript, &fixture.new_bytes).unwrap();
    let settings = options();
    settings.cancel.cancel();
    evidence.equal(
        "cancellation remains an error rather than a partial success",
        json!(matches!(
            archive.backup(std::slice::from_ref(&fixture.source), &settings),
            Err(Error::Cancelled)
        )),
        json!(true),
    );
    evidence.equal(
        "cancelled capture adds no complete raw revision",
        json!(archive.revisions(&previous.source_id, 100, None).unwrap().items.len()),
        json!(1),
    );
    evidence.equal(
        "cancelled capture does not replace the visible session",
        json!(archive.session(&previous.id).unwrap().revision_id),
        json!(previous.revision_id),
    );
    evidence.equal(
        "previous bytes remain recoverable",
        json!(raw_bytes(&archive, previous.revision_id) == fixture.old_bytes),
        json!(true),
    );
    evidence.equal(
        "previous search stays available",
        json!(archive.search(&query("baselineword")).unwrap().total),
        json!(1),
    );
}
