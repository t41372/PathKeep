//! Backup and restart regressions for damage confined to one reader publication.
use super::*;

const BASE: &str = "11111111-1111-4111-8111-111111111111";
const CHILD: &str = "22222222-2222-4222-8222-222222222222";
const LATER: &str = "33333333-3333-4333-8333-333333333333";

fn header(id: &str) -> serde_json::Value {
    serde_json::json!({"type":"session_meta","payload":{"id":id,"cwd":"/project"}})
}
fn rollout(dir: &std::path::Path, id: &str, rows: &str) -> SourceRoot {
    let root = dir.join(id);
    fs::create_dir_all(root.join("sessions")).unwrap();
    fs::write(root.join(format!("sessions/rollout-2026-11-10T12-00-00-{id}.jsonl")), rows).unwrap();
    SourceRoot { root, harness: Harness::Codex }
}
fn source(a: &Archive, id: &str) -> (String, String) {
    let id = id.to_owned();
    a.store
        .read(DB, move |tx| {
            tx.query_one(
                "SELECT id,conversation_id FROM ah_sessions WHERE native_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
        .unwrap()
}

#[test]
fn obsolete_damaged_hash_cannot_block_attach_or_collection() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("archive");
    let a = Archive::create_unencrypted(&path).unwrap();
    let first = format!("{}\n", header(BASE));
    let base = rollout(t.path(), BASE, &first);
    let later = rollout(t.path(), LATER, &format!("{}\n", header(LATER)));
    a.backup(&[base.clone(), later], &Default::default()).unwrap();
    let (_, healthy) = source(&a, LATER);
    let pin = a.jobs.pin_publication().unwrap();
    rollout(t.path(), BASE, &format!("{first}{{}}\n"));
    a.backup(&[base], &Default::default()).unwrap();
    let changed=a.store.write(DB,Priority::Batch,|tx| tx.execute(
        "UPDATE ah_reader_page SET hash='wrong storage class' WHERE rowid=(SELECT p.rowid
         FROM ah_reader_page p JOIN ah_reader_version v ON v.token=p.owner WHERE v.state='retired' LIMIT 1)",()
    )).unwrap();
    assert_eq!(changed, 1);
    drop(pin);
    drop(a);
    let a = Archive::open(&path, b"").expect("obsolete reader data blocked attachment");
    assert_eq!(a.reader_header(&healthy).unwrap().physical_records, 1);
    a.collect_reader_garbage(&Default::default()).unwrap();
    let retired = a
        .store
        .read(DB, |tx| {
            tx.query_one("SELECT count(*) FROM ah_reader_version WHERE state='retired'", (), |r| {
                Ok(r.get::<i64>(0)?)
            })
        })
        .unwrap();
    assert_eq!(retired, 0);
}

#[test]
fn parked_continuation_cannot_interrupt_capture_of_later_root() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let first = format!("{}\n", header(BASE));
    let base = rollout(t.path(), BASE, &first);
    let mut meta = header(CHILD);
    meta["payload"]["history_base"] = serde_json::json!({"thread_id":BASE,"end_ordinal_exclusive":1,"end_byte_offset":first.len()});
    let child = rollout(t.path(), CHILD, &format!("{meta}\n"));
    a.backup(&[base.clone(), child.clone()], &Default::default()).unwrap();
    fs::remove_dir_all(&child.root).unwrap();
    let (child_source, conversation) = source(&a, CHILD);
    a.store.write(DB,Priority::Batch,move|tx|{
        let n=tx.execute("DELETE FROM ah_reader_page WHERE family=?1 AND owner=(SELECT token FROM ah_reader_version WHERE source=?2 AND state='published')",params![RANK,child_source.as_str()])?;
        assert!(n>0);
        refresh(tx,&child_source,None)
    }).unwrap();
    assert!(a.recover_reader_heads(&Default::default()).unwrap());
    assert!(matches!(a.reader_header(&conversation), Err(Error::ReaderRebuildRequired)));
    rollout(t.path(), BASE, &format!("{first}{{}}\n"));
    let later = rollout(t.path(), LATER, &format!("{}\n", header(LATER)));
    let roots = [base, later];
    let outcome = a.backup(&roots, &Default::default());
    let later_saved=a.store.read(Db::AgentHistoryEvidence,|tx|tx.query_one(
        "SELECT count(*) FROM ah_sources WHERE relative_path LIKE '%333333333333.jsonl' AND latest_revision_id IS NOT NULL",(),|r|Ok(r.get::<i64>(0)?))).unwrap();
    assert_eq!(later_saved, 1, "later root was skipped: {outcome:?}");
    assert_eq!(outcome.unwrap().captured, 2);
    let (source_id, base_conversation) = source(&a, BASE);
    let h = a.reader_header(&base_conversation).unwrap();
    let member = a.reader_sources(&h, None, 1).unwrap().remove(0);
    assert!(a.load_versioned_parse_report(member.revision, &member.parser).unwrap().is_some());
    let attempt = a.current_projection_id(&source_id).unwrap().unwrap();
    a.backup(&roots, &Default::default()).unwrap();
    assert_eq!(a.current_projection_id(&source_id).unwrap(), Some(attempt));
    assert!(matches!(a.reader_header(&conversation), Err(Error::ReaderRebuildRequired)));
    assert_eq!(a.reader_header(&source(&a, LATER).1).unwrap().physical_records, 1);
}

#[test]
fn a_reader_build_error_preserves_parse_evidence_and_later_files() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let first = format!("{}\n", header(BASE));
    let base = rollout(t.path(), BASE, &first);
    a.backup(std::slice::from_ref(&base), &Default::default()).unwrap();
    // This disposable index rejects just the second version of an existing source.
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute(
                "CREATE UNIQUE INDEX reject_second_reader_version ON ah_reader_version(source)",
                (),
            )
        })
        .unwrap();
    let (base_id, _) = source(&a, BASE);
    assert!(
        a.reparse_source(&base_id, &Default::default()).is_err(),
        "explicit reparse must report its own build failure"
    );
    rollout(t.path(), BASE, &format!("{first}{{}}\n"));
    let later = rollout(t.path(), LATER, &format!("{}\n", header(LATER)));
    let roots = [base, later];
    let outcome = a.backup(&roots, &Default::default());
    let (base_id, conversation) = source(&a, BASE);
    let latest = a.latest_revision(&base_id).unwrap().unwrap();
    let reports = a
        .store
        .read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one(
                "SELECT count(*) FROM ah_parse_reports WHERE revision_id=?1",
                [latest.id],
                |r| Ok(r.get::<i64>(0)?),
            )
        })
        .unwrap();
    assert_eq!(reports, 1, "reader work preceded the parse evidence: {outcome:?}");
    assert_eq!(a.reader_header(&source(&a, LATER).1).unwrap().physical_records, 1);
    assert!(matches!(a.reader_header(&conversation), Err(Error::ReaderRebuildRequired)));
    let attempt = a.current_projection_id(&base_id).unwrap().unwrap();
    assert_eq!(outcome.unwrap().captured, 2);
    a.backup(&roots, &Default::default()).unwrap();
    assert_eq!(a.current_projection_id(&base_id).unwrap(), Some(attempt));
    a.store
        .write(DB, Priority::Batch, |tx| tx.execute("DROP INDEX reject_second_reader_version", ()))
        .unwrap();
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    assert_eq!(a.reader_header(&conversation).unwrap().physical_records, 2);
}

#[test]
fn an_undeletable_retirement_does_not_block_other_owners() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("archive");
    let a = Archive::create_unencrypted(&path).unwrap();
    let root = rollout(t.path(), BASE, &format!("{}\n", header(BASE)));
    a.backup(&[root], &Default::default()).unwrap();
    let (_, healthy) = source(&a, BASE);
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute(
                "INSERT INTO ah_reader_retired(publication) VALUES('blocked'),('other')",
                (),
            )?;

            Ok(())
        })
        .unwrap();
    drop(a);
    let db = rusqlite::Connection::open(path.join("derived/agent-history-read-model.db")).unwrap();
    db.execute_batch("CREATE TRIGGER keep_obsolete BEFORE DELETE ON ah_reader_retired WHEN OLD.publication='blocked' BEGIN SELECT RAISE(ABORT,'blocked retirement'); END;").unwrap();
    drop(db);
    let a = Archive::open(&path, b"").expect("collection failure blocked attach");
    assert_eq!(a.reader_header(&healthy).unwrap().physical_records, 1);
    let left = a
        .store
        .read(DB, |tx| {
            tx.query("SELECT publication FROM ah_reader_retired", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(left, ["blocked"]);
}
