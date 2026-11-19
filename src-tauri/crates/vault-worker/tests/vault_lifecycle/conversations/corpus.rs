//! Builds the independent native-byte and conversation oracle for archive operations.
//! Worker sessions and maintenance requests belong to the scenarios.

use super::*;
use agent_history::{SearchQuery, archive::ConversationMark, query::SessionQuery};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[allow(dead_code)]
#[path = "../../../../agent-history/tests/support/fixtures.rs"]
pub(super) mod fixtures;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

pub(super) fn query() -> SearchQuery {
    SearchQuery {
        text: fixtures::PRIVATE_TEXT.into(),
        harness: None,
        project_path: None,
        limit: 100,
    }
}

pub(super) fn sessions(archive: &Archive) -> BTreeMap<String, serde_json::Value> {
    let mut sessions = BTreeMap::new();
    for trash in ["only", "exclude"] {
        let page = archive
            .list_sessions(&SessionQuery {
                trash: trash.into(),
                put_away: "include".into(),
                ..Default::default()
            })
            .unwrap();
        assert!(page.next_cursor.is_none());
        for item in page.items {
            assert_eq!(item.trashed, trash == "only");
            assert!(
                sessions
                    .insert(item.harness.to_string(), serde_json::to_value(item).unwrap())
                    .is_none()
            );
        }
    }
    sessions
}

pub(super) struct Corpus {
    sessions: BTreeMap<String, serde_json::Value>,
    receipt_fingerprint: String,
    capture_facts: Vec<(String, String, String)>,
    // Native file names (including DeepSeek's older generation), newest bytes first.
    revisions: BTreeMap<PathBuf, Vec<Vec<u8>>>,
    removed: PathBuf,
    revision_ids: BTreeMap<PathBuf, Vec<i64>>,
}

impl Corpus {
    pub(super) fn seed(fixture: &Fixture) -> Self {
        let archive = fixture.vault().agent_history().unwrap();
        let logs = fixtures::Fixture::four_harnesses(fixture.sandbox.path());
        // A tail after 32 MiB in the same native frame can only open under the
        // one-byte replay budget below if the operation carried the rendered pages.
        let codex = &logs.files[1];
        let mut decoded = zstd::stream::decode_all(fs::read(codex).unwrap().as_slice()).unwrap();
        for _ in 0..4 {
            decoded.resize(decoded.len() + 8 * 1024 * 1024, b' ');
            decoded.push(b'\n');
        }
        decoded.extend_from_slice(br#"{"type":"response_item","payload":{"type":"message","id":"lifecycle-tail","role":"user","content":[{"type":"input_text","text":"reader page survived"}]}}"#);
        decoded.push(b'\n');
        fs::write(codex, zstd::stream::encode_all(decoded.as_slice(), 1).unwrap()).unwrap();
        drop(decoded);
        let roots = logs
            .roots
            .iter()
            .map(|(name, path)| SourceRoot { harness: name.parse().unwrap(), root: path.clone() })
            .collect::<Vec<_>>();
        let commits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = commits.clone();
        let options = BackupOptions {
            read_git: false,
            progress: Some(Arc::new(move |progress| {
                if progress.phase == "raw_committed" {
                    observed
                        .lock()
                        .unwrap()
                        .push((progress.source_id, progress.revision_id.unwrap()));
                }
            })),
            ..Default::default()
        };
        let changed = &logs.files[0];
        let mut paths = logs.files.clone();
        paths.push(logs.files[3].with_file_name("session.v3.jsonl"));
        let mut revisions = paths
            .iter()
            .map(|path| (fs::canonicalize(path).unwrap(), vec![fs::read(path).unwrap()]))
            .collect::<BTreeMap<_, _>>();
        archive.backup(&roots, &options).unwrap();
        // JSON whitespace keeps the native record parseable across the raw slice boundary.
        let mut bytes = fs::read(changed).unwrap();
        bytes.pop();
        bytes.resize(4 * 1024 * 1024 + 100, b' ');
        bytes.push(b'\n');
        fs::write(changed, &bytes).unwrap();
        revisions.get_mut(&fs::canonicalize(changed).unwrap()).unwrap().insert(0, bytes);
        let removed = fs::canonicalize(&logs.files[2]).unwrap();
        fs::remove_file(&removed).unwrap();
        archive.backup(&roots, &options).unwrap();
        let mut initial = sessions(&archive);
        assert_eq!(
            initial.keys().map(String::as_str).collect::<Vec<_>>(),
            ["claude-code", "codex", "deepseek", "pi"]
        );
        for item in initial.values() {
            assert_eq!(item["native_id"], fixtures::SHARED_ID);
            assert_eq!(item["project_path"], logs.project);
            assert_eq!(item["starred"], false);
            assert_eq!(item["put_away"], false);
            assert_eq!(item["trashed"], false);
        }
        assert_eq!(initial["pi"]["only_in_archive"], true);
        let claude = initial.get_mut("claude-code").unwrap();
        archive
            .mark_conversation(
                claude["id"].as_str().unwrap(),
                &ConversationMark {
                    starred: Some(true),
                    title: Some(Some("Saved title".into())),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        claude["starred"] = true.into();
        claude["title"] = "Saved title".into();
        claude["title_source"] = "user".into();
        let codex = initial.get_mut("codex").unwrap();
        let marked = archive
            .mark_conversation(
                codex["id"].as_str().unwrap(),
                &ConversationMark { trash: Some(true), ..Default::default() },
                true,
            )
            .unwrap();
        codex["trashed"] = true.into();
        codex["trashed_at"] = marked.trashed_at.expect("trash time").into();
        assert_eq!(sessions(&archive), initial);

        // Use IDs issued at commit, including superseded revisions. A listing that
        // forgets an old revision must not silently shrink the expected export set.
        let mut revision_ids: BTreeMap<PathBuf, Vec<i64>> = BTreeMap::new();
        for (source_id, revision) in commits.lock().unwrap().iter().rev() {
            let source = archive.source(source_id).unwrap();
            let path = Path::new(&source.root).join(&source.relative_path);
            revision_ids.entry(path).or_default().push(*revision);
        }
        assert_eq!(revisions.keys().collect::<Vec<_>>(), revision_ids.keys().collect::<Vec<_>>());
        let receipt_fingerprint = receipts::seed(fixture);
        let capture_facts = captured_units(&fixture.vault());
        assert!(!capture_facts.is_empty());
        fs::remove_dir_all(logs.source_tree).unwrap();
        Self {
            sessions: initial,
            revisions,
            removed,
            revision_ids,
            capture_facts,
            receipt_fingerprint,
        }
    }

    pub(super) fn check(&self, vault: &Vault, password: Option<&str>) {
        let archive = vault.agent_history().unwrap();
        assert_eq!(sessions(&archive), self.sessions);
        let head = archive.reader_header(self.sessions["codex"]["id"].as_str().unwrap()).unwrap();
        let job =
            agent_history::read_model::JobControl { max_decoded_bytes: 1, ..Default::default() };
        let tail = archive.reader_window(&head, None, head.physical_records - 1, 1, &job).unwrap();
        assert_eq!(tail[0].native_id.as_deref(), Some("lifecycle-tail"));
        assert_eq!(tail[0].parts[0].text, "reader page survived");
        assert_eq!(captured_units(vault), self.capture_facts);
        assert_eq!(receipts::fingerprint(vault), self.receipt_fingerprint);
        let sources = archive.list_sources(&Default::default()).unwrap();
        assert!(sources.next_cursor.is_none());
        let mut seen = BTreeSet::new();
        for source in sources.items {
            let path = Path::new(&source.root).join(&source.relative_path);
            assert!(seen.insert(path.clone()));
            assert_eq!(source.missing_since.is_some(), path == self.removed);
            let expected = &self.revisions[&path];
            let revisions = archive.revisions(&source.id, 100, None).unwrap();
            assert!(revisions.next_cursor.is_none());
            assert_eq!(revisions.items.len(), expected.len(), "{path:?}");
            let ids = &self.revision_ids[&path];
            assert!(revisions.items.iter().map(|r| r.id).eq(ids.iter().copied()));
            for (&revision, expected) in ids.iter().zip(expected) {
                let mut bytes = Vec::new();
                archive.write_raw(revision, &mut bytes).unwrap();
                assert!(bytes == *expected, "revision {revision} of {path:?}");
            }
        }
        assert_eq!(seen, self.revisions.keys().cloned().collect());
        archive.synchronize_search().unwrap();
        let found = archive
            .search(&query())
            .unwrap()
            .items
            .into_iter()
            .map(|hit| hit.session_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            found,
            ["claude-code", "deepseek", "pi"]
                .map(|name| self.sessions[name]["id"].as_str().unwrap().to_owned())
                .into()
        );
        let root = &vault.paths().app_root;
        assert!(!root.join("agent-history-format").exists());
        assert!(!root.join("agent-history-key.envelope").exists());
        if let Some(password) = password {
            use tantivy::Directory;
            let key = Vault::unlock_data_key(vault.paths(), password).unwrap();
            let index = vault_crypto::EncryptedDirectory::open_for(
                &root.join("derived/agent-history-search"),
                &key,
                vault_crypto::KeyPurpose::AgentHistorySearchIndex,
                0,
            )
            .unwrap();
            let meta = index.atomic_read(Path::new("meta.json")).unwrap();
            assert!(
                serde_json::from_slice::<serde_json::Value>(&meta).unwrap()["segments"].is_array()
            );
            for path in expected_root::files_under(root).unwrap() {
                if let Some(bytes) = read_unless_locked(root, &path) {
                    assert!(
                        !bytes
                            .windows(fixtures::PRIVATE_TEXT.len())
                            .any(|word| word == fixtures::PRIVATE_TEXT.as_bytes()),
                        "plaintext conversation in {path}"
                    );
                }
            }
        }
    }

    pub(super) fn retitle(&mut self, vault: &Vault) {
        let item = self.sessions.get_mut("claude-code").unwrap();
        vault
            .agent_history()
            .unwrap()
            .mark_conversation(
                item["id"].as_str().unwrap(),
                &ConversationMark {
                    title: Some(Some("After the copy".into())),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        item["title"] = "After the copy".into();
    }
}

fn captured_units(vault: &Vault) -> Vec<(String, String, String)> {
    vault.store().read(vault_store::Db::AgentHistoryEvidence,|s|s.query(
        "SELECT b.public_id,u.conversation_id,u.outcome FROM ah_run_conversations u JOIN ah_backup_runs b ON b.id=u.run_id ORDER BY b.public_id,u.conversation_id",
        (),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?)))).unwrap()
}
