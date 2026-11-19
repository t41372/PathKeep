//! Responsibilities: prove changing native inventory roles cannot duplicate visible chats.
//! Not responsible for: mocking adapter discovery or changing retained raw evidence.
//! Failure mode specified first: Cowork audit is indexed as a fallback, then stays indexed
//! after a main transcript appears and discovery correctly calls the audit metadata.

use agent_history::{
    Archive, Harness, SourceRoot, capture::BackupOptions, query::SessionQuery, search::SearchQuery,
};
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path};
use vault_crypto::KdfParams;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

struct Evidence(Value);

impl Drop for Evidence {
    fn drop(&mut self) {
        self.0["outcome"] = json!(if std::thread::panicking() { "failed" } else { "passed" });
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../artifacts/agent-history/capture-roles");
        let _ = fs::create_dir_all(&directory);
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.0) {
            let _ = fs::write(directory.join("cowork-fallback.json"), bytes);
        }
    }
}

#[test]
fn a_cowork_main_transcript_retires_its_audit_fallback_without_erasing_bytes() {
    let mut evidence = Evidence(json!({
        "seed":"cowork-role-transition-v1","private_data":false,
        "reproduce":"cargo test --manifest-path src-tauri/Cargo.toml -p agent-history --test capture_roles",
    }));
    let temporary = tempfile::tempdir().unwrap();
    let source =
        SourceRoot { harness: Harness::ClaudeCode, root: temporary.path().join("native-claude") };
    let body = source.root.join("local-agent-mode-sessions/account/org/local_task");
    let audit_path = body.join("audit.jsonl");
    let audit = format!(
        "{}\n{}\n",
        json!({"type":"system","subtype":"init","session_id":"native-cowork",
            "cwd":"/synthetic/task","_audit_timestamp":"2026-11-11T00:00:00Z"}),
        json!({"type":"assistant","session_id":"native-cowork","uuid":"reply",
            "message":{"role":"assistant","content":[{"type":"text","text":"onecopyneedle"}]},
            "_audit_timestamp":"2026-11-11T00:00:01Z"}),
    );
    write(&audit_path, audit.as_bytes());
    // A metadata-only Desktop card tree can contain identically named files.
    write(
        &source.root.join("claude-code-sessions/account/org/local_other/audit.jsonl"),
        audit.as_bytes(),
    );
    let archive = Archive::create_with_params(
        &temporary.path().join("archive"),
        b"synthetic-role-password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    let root = archive.root().to_owned();
    drop(archive);
    let model = root.join(vault_layout::database_path(vault_store::Db::AgentHistoryReadModel));
    let empty = fs::read(&model).unwrap();
    let archive = Archive::open(&root, b"synthetic-role-password").unwrap();
    archive.backup(std::slice::from_ref(&source), &BackupOptions::default()).unwrap();
    let before = archive.list_sessions(&SessionQuery::default()).unwrap().items;
    evidence.0["before_sessions"] = json!(before.len());
    assert_eq!(before.len(), 1);
    let raw_revision = before[0].revision_id;
    let transcript = format!(
        "{}\n",
        json!({"type":"assistant","sessionId":"native-cowork","uuid":"reply",
            "cwd":"/synthetic/task","entrypoint":"local-agent",
            "message":{"role":"assistant","content":[{"type":"text","text":"onecopyneedle"}]},
            "timestamp":"2026-11-11T00:00:01Z"}),
    );
    write(&body.join(".claude/projects/project/main.jsonl"), transcript.as_bytes());
    archive.backup(std::slice::from_ref(&source), &BackupOptions::default()).unwrap();
    let current = archive.list_sessions(&SessionQuery::default()).unwrap().items;
    evidence.0["after_sessions"] = json!(current.len());
    assert_eq!(current.len(), 1, "raw-only audit must stop being a searchable transcript");
    assert_ne!(current[0].source_id, before[0].source_id);
    let descriptor = archive.source(&before[0].source_id).unwrap();
    assert_eq!(descriptor.kind, "metadata");
    // Recreate the older descriptor format while preserving both capture runs.
    // The same offline decision must be recoverable without another source scan.
    archive
        .register_source(
            &descriptor.id,
            &SourceRoot { harness: source.harness, root: descriptor.root.into() },
            &agent_history::Artifact {
                path: audit_path.clone(),
                relative_path: descriptor.relative_path,
                kind: agent_history::ArtifactKind::Transcript,
                project_hint: None,
                generation: None,
                current: true,
            },
        )
        .unwrap();
    assert!(
        archive.reparse_source(&before[0].source_id, &BackupOptions::default()).is_err(),
        "retired audit was reparsed"
    );
    let found = archive
        .search(&SearchQuery {
            text: "onecopyneedle".into(),
            harness: Some(Harness::ClaudeCode),
            project_path: None,
            limit: 10,
        })
        .unwrap();
    evidence.0["search_hits"] = json!(found.total);
    assert_eq!(found.total, 1);
    let mut retained = Vec::new();
    archive.raw_reader(raw_revision).unwrap().read_to_end(&mut retained).unwrap();
    evidence.0["raw_bytes_retained"] = json!(retained == audit.as_bytes());
    assert_eq!(retained, audit.as_bytes());
    assert_eq!(fs::read(audit_path).unwrap(), audit.as_bytes());
    drop(archive);
    fs::remove_dir_all(&source.root).unwrap();
    fs::remove_dir_all(vault_layout::path(&root, vault_layout::ArtifactId::AgentHistorySearch))
        .unwrap();
    fs::write(model, empty).unwrap();
    let rebuilt = Archive::open(&root, b"synthetic-role-password").unwrap();
    rebuilt.rebuild_from_retained_evidence(&Default::default()).unwrap();
    let rows = rebuilt.list_sessions(&Default::default()).unwrap().items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source_id, current[0].source_id);
    let search = rebuilt
        .search(&SearchQuery {
            text: "onecopyneedle".into(),
            harness: Some(Harness::ClaudeCode),
            project_path: None,
            limit: 10,
        })
        .unwrap();
    assert_eq!(search.total, 1);
    assert_eq!(search.items[0].session_id, rows[0].id);
    evidence.0["rebuilt_search_hits"] = json!(search.total);
    let mut bytes = Vec::new();
    rebuilt.write_raw(raw_revision, &mut bytes).unwrap();
    assert_eq!(bytes, audit.as_bytes());
}

#[test]
fn seeing_an_unreadable_cowork_main_keeps_the_saved_audit_searchable() {
    use std::sync::{Arc, Mutex};

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let source = SourceRoot { harness: Harness::ClaudeCode, root: temp.path().join("cowork") };
    let body = source.root.join("local-agent-mode-sessions/account/org/local_task");
    let audit = body.join("audit.jsonl");
    let main = body.join(".claude/projects/project/main.jsonl");
    let raw = format!(
        "{}\n{}\n",
        json!({"type":"system","subtype":"init","session_id":"saved","cwd":"/task","_audit_timestamp":"2026-11-11T00:00:00Z"}),
        json!({"type":"assistant","session_id":"saved","uuid":"reply","message":{"role":"assistant","content":[{"type":"text","text":"savedonlyneedle"}]},"_audit_timestamp":"2026-11-11T00:00:01Z"})
    );
    write(&audit, raw.as_bytes());
    drop(
        Archive::create_with_params(
            &root,
            b"test",
            KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
        )
        .unwrap(),
    );
    let model = root.join(vault_layout::database_path(vault_store::Db::AgentHistoryReadModel));
    let empty_model = fs::read(&model).unwrap();
    let archive = Archive::open(&root, b"test").unwrap();
    let backup =
        |options: BackupOptions| archive.backup(std::slice::from_ref(&source), &options).unwrap();
    // Discovery emits the main first, then the audit. Remove each selected file
    // at its file_found boundary so both sightings survive the failed reads.
    let lose_files = |paths: Vec<std::path::PathBuf>| {
        let paths = Mutex::new(paths.into_iter());
        BackupOptions {
            progress: Some(Arc::new(move |event| {
                if event.phase == "file_found"
                    && let Some(path) = paths.lock().unwrap().next()
                {
                    fs::remove_file(path).unwrap();
                }
            })),
            ..Default::default()
        }
    };
    backup(Default::default());
    let saved = archive.list_sessions(&Default::default()).unwrap().items.pop().unwrap();
    write(&main, raw.as_bytes());
    let failed = backup(lose_files(vec![main.clone(), audit.clone()]));
    assert_eq!((failed.captured, failed.unchanged), (0, 0));
    assert_eq!(failed.issues.iter().filter(|i| i.code == "source_capture_unavailable").count(), 2);
    let assert_published = |archive: &Archive| {
        let sessions = archive.list_sessions(&Default::default()).unwrap().items;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].source_id, saved.source_id);
        let found = archive
            .search(&SearchQuery {
                text: "savedonlyneedle".into(),
                harness: None,
                project_path: None,
                limit: 10,
            })
            .unwrap();
        assert_eq!(found.total, 1);
        assert_eq!(found.items[0].session_id, saved.id);
    };
    archive.reparse_source(&saved.source_id, &Default::default()).unwrap();
    assert_published(&archive);
    drop(archive);
    fs::remove_dir_all(&source.root).unwrap();
    fs::remove_dir_all(vault_layout::path(&root, vault_layout::ArtifactId::AgentHistorySearch))
        .unwrap();
    fs::write(model, empty_model).unwrap();
    let rebuilt = Archive::open(&root, b"test").unwrap();
    rebuilt.rebuild_from_retained_evidence(&Default::default()).unwrap();
    assert_published(&rebuilt);
    write(&audit, raw.as_bytes());
    write(&main, raw.as_bytes());
    let audit_only =
        rebuilt.backup(std::slice::from_ref(&source), &lose_files(vec![main])).unwrap();
    assert_eq!(audit_only.unchanged, 1);
    assert_published(&rebuilt);
}
