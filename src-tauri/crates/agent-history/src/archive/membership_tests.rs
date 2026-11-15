//! Responsibilities: bound SQL parameter counts for a conversation with many retained copies.
//! Not responsible for: measuring transcript decoding or replacing real database queries.
//! Failure mode: one conversation's source array exceeds the existing 500-item SQL page bound.
use super::Archive;
use crate::{Artifact, ArtifactKind, Harness, SessionMetadata, SourceRoot};
use sha2::{Digest, Sha256};
use vault_store::{Db, Priority, Session, params};

fn clone_sql(tx: &Session<'_>, table: &str, replacements: &[(&str, &str)]) -> String {
    let names = tx.table_columns(table).unwrap();
    let selected = names
        .iter()
        .map(|name| {
            replacements
                .iter()
                .find(|(column, _)| name == column)
                .map_or_else(|| name.clone(), |(_, value)| value.to_string())
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("INSERT INTO {table} SELECT {selected} FROM {table} WHERE id=?3")
}

#[test]
fn one_conversations_sources_cannot_grow_sql_parameters_past_the_page_bound() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(temp.path()).unwrap();
    let source = SourceRoot { harness: Harness::Codex, root: "/synthetic/codex".into() };
    let artifact = Artifact {
        path: "/synthetic/codex/first.jsonl".into(),
        relative_path: "first.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let bytes = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"many-copies\"}}\n";
    let raw = archive.begin_revision("first", &source, &artifact).unwrap();
    archive.append_chunk(raw, 0, bytes.to_vec()).unwrap();
    let raw = archive
        .finish_revision(raw, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id;
    let attempt = archive.prepare_projection("first", raw, "synthetic", false).unwrap().unwrap();
    archive
        .publish_projection_attempt(
            &attempt,
            &SessionMetadata { native_id: "many-copies".into(), ..Default::default() },
            true,
        )
        .unwrap();
    let id = archive.list_sessions(&Default::default()).unwrap().items.remove(0).id;
    // These are complete synthetic captures of the same bytes: separate source,
    // revision and publication handles, sharing the authenticated slice manifest.
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let sources = clone_sql(
                tx,
                "ah_sources",
                &[("id", "?1"), ("latest_revision_id", "?2"), ("relative_path", "?1||'.jsonl'")],
            );
            let revisions = clone_sql(tx, "ah_revisions", &[("id", "?2"), ("source_id", "?1")]);
            let mut source_insert = tx.prepare(&sources)?;
            let mut revision_insert = tx.prepare(&revisions)?;
            let mut manifest = tx.prepare(
                "INSERT INTO ah_revision_chunks
             SELECT ?1,ordinal,chunk_hash,byte_len,byte_offset
             FROM ah_revision_chunks WHERE revision_id=?2",
            )?;
            for index in 1..=600 {
                let source = format!("copy-{index}");
                let revision = raw + index;
                source_insert.execute(params![source.clone(), revision, "first"])?;
                revision_insert.execute(params![source, revision, raw])?;
                manifest.execute(params![revision, raw])?;
            }
            Ok(())
        })
        .unwrap();
    archive
        .store
        .write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            let sql = clone_sql(
                tx,
                "ah_sessions",
                &[
                    ("id", "?1"),
                    ("source_id", "?1"),
                    ("revision_id", "?2"),
                    ("projection_id", "?2"),
                    ("relative_path", "?1||'.jsonl'"),
                    ("conversation_current", "0"),
                    ("copy_of_source_id", "?3"),
                ],
            );
            let mut insert = tx.prepare(&sql)?;
            for index in 1..=600 {
                insert.execute(params![format!("copy-{index}"), raw + index, "first"])?;
            }
            Ok(())
        })
        .unwrap();
    archive.store.capture_query_plans(Db::AgentHistoryEvidence, true).unwrap();
    let view = archive.session(&id).unwrap();
    assert_eq!(view.sources.len(), 601);
    let plans = archive.store.take_query_plans(Db::AgentHistoryEvidence).unwrap();
    assert!(
        plans.iter().all(|plan| plan.sql.matches('?').count() <= 500),
        "membership reads must stay within the fixed page parameter bound"
    );
}
