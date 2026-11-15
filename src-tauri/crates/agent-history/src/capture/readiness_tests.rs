//! Responsibilities: distinguish retained-storage corruption from an unreadable native transport.
//! Not responsible for: mocking the parser or testing source-file permissions.
//! Failure mode: the first-record gate swallows a raw integrity error as a parser limitation.
use super::{BackupOptions, readiness};
use crate::{Archive, Artifact, ArtifactKind, Harness, SourceRoot};
use sha2::{Digest, Sha256};
use vault_store::{Db, Priority};

#[test]
fn first_record_gating_propagates_retained_integrity_failure() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(temp.path()).unwrap();
    let source = SourceRoot { harness: Harness::Pi, root: "/synthetic/pi".into() };
    let artifact = Artifact {
        path: "/synthetic/pi/chat.jsonl".into(),
        relative_path: "chat.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let bytes = b"{\"type\":\"session\",\"id\":\"synthetic\"}\n";
    let raw = archive.begin_revision("synthetic-source", &source, &artifact).unwrap();
    archive.append_chunk(raw, 0, bytes.to_vec()).unwrap();
    let revision = archive
        .finish_revision(raw, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id;
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |tx| {
            tx.execute("UPDATE ah_chunks SET codec='invalid'", ())?;
            Ok(())
        })
        .unwrap();
    let result = readiness::first_record(&archive, &artifact, revision, &BackupOptions::default());
    match result {
        Err(error) => assert!(error.to_string().contains("unknown slice codec")),
        Ok(_) => panic!("retained corruption must not become a source limitation"),
    }
}
