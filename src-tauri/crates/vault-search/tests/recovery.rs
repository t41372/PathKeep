//! Responsibilities: prove durable sequence, rebuild, version, integrity and memory contracts.
//! Not responsible for: canonical outbox transactions or cryptographic algorithms.
//! Failure modes: a stale batch rolls back the watermark; duplicate upserts survive;
//! an incomplete/cancelled rebuild replaces the old index; missing/versioned/corrupt
//! files are mistaken for an empty index; reload/cache settings are ignored.
mod support;
use std::path::Path;
use support::*;
use tantivy::Directory;
use vault_crypto::EncryptedDirectory;
use vault_search::*;

#[test]
fn apply_reopen_and_replay_preserve_unique_pages_and_committed_sequence() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), vec![page(1, "old"), page(2, "deleted")]);
    index
        .apply(Batch {
            last_seq: 70,
            upserts: vec![page(1, "sentinelprivate"), page(3, "new"), page(3, "replacement")],
            deletes: vec![2],
        })
        .unwrap();
    assert_eq!(index.last_applied_seq().unwrap(), 70);
    drop(index);
    let index = SearchIndex::open(tmp.path(), Some(&key()), config()).unwrap();
    assert_eq!(index.last_applied_seq().unwrap(), 70);
    index.apply(Batch { last_seq: 70, upserts: vec![page(4, "stale")], deletes: vec![1] }).unwrap();
    assert_eq!(ids(&session(&index, Ast::And(vec![]))), vec![1, 3]);
    assert_eq!(ids(&session(&index, term("replacement"))), vec![3]);
    assert_eq!(session(&index, term("old")).count().unwrap(), 0);
    index.apply(Batch { last_seq: 71, upserts: vec![], deletes: vec![1] }).unwrap();
    assert_eq!(index.last_applied_seq().unwrap(), 71);
    assert_no_plaintext(tmp.path());
}

#[test]
fn rebuild_serves_the_old_index_until_atomic_publication() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), vec![page(1, "old")]);
    let old = session(&index, term("old"));
    let mut progress = vec![];
    index
        .rebuild(
            (10..3100).map(|id| {
                assert_eq!(session(&index, term("old")).count().unwrap(), 1);
                Ok(page(id, "new"))
            }),
            99,
            |p| progress.push(p),
            || false,
        )
        .unwrap();
    assert_eq!(old.count().unwrap(), 1);
    assert_eq!(session(&index, term("old")).count().unwrap(), 0);
    assert_eq!(session(&index, term("new")).count().unwrap(), 3090);
    assert_eq!(index.last_applied_seq().unwrap(), 99);
    assert_eq!(progress.last().unwrap().documents, 3090);
    assert!(progress.windows(2).all(|p| p[0].documents <= p[1].documents));
}

#[test]
fn cancelled_failed_and_duplicate_streams_never_publish_and_writer_recovers() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), vec![page(1, "old")]);
    let count = std::cell::Cell::new(0);
    let result = index.rebuild(
        (2..5000).map(|i| Ok(page(i, "new"))),
        50,
        |_| {},
        || {
            count.set(count.get() + 1);
            count.get() > 1500
        },
    );
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(
        index
            .rebuild(
                [Ok(page(2, "new")), Err(Error::Input("source failed".into()))],
                50,
                |_| {},
                || false
            )
            .is_err()
    );
    assert!(
        index
            .rebuild([Ok(page(2, "new")), Ok(page(2, "duplicate"))], 50, |_| {}, || false)
            .is_err()
    );
    assert_eq!(index.last_applied_seq().unwrap(), 1);
    assert_eq!(ids(&session(&index, Ast::And(vec![]))), vec![1]);
    index.apply(Batch { last_seq: 2, upserts: vec![page(3, "after")], deletes: vec![] }).unwrap();
    assert_eq!(ids(&session(&index, Ast::And(vec![]))), vec![1, 3]);
}

#[test]
fn missing_versions_and_corruption_report_rebuild_needed_with_integrity_cause() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(matches!(
        SearchIndex::open(tmp.path(), Some(&key()), config()),
        Err(Error::RebuildNeeded { .. })
    ));
    let index = create(tmp.path(), vec![page(1, "sentinelprivate")]);
    drop(index);
    let dir = EncryptedDirectory::open(tmp.path(), &key(), 1024).unwrap();
    let original = dir.atomic_read(Path::new("meta.json")).unwrap();
    for key_name in ["schema_version", "analyzer_version"] {
        let mut meta: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let mut payload: serde_json::Value =
            serde_json::from_str(meta["payload"].as_str().unwrap()).unwrap();
        payload[key_name] = 999.into();
        meta["payload"] = serde_json::to_string(&payload).unwrap().into();
        dir.atomic_write(Path::new("meta.json"), &serde_json::to_vec(&meta).unwrap()).unwrap();
        assert!(matches!(
            SearchIndex::open(tmp.path(), Some(&key()), config()),
            Err(Error::RebuildNeeded { .. })
        ));
    }
    dir.atomic_write(Path::new("meta.json"), &original).unwrap();
    let pos = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "pos"))
        .unwrap();
    let bytes = std::fs::read(&pos).unwrap();
    std::fs::write(&pos, &bytes[..20]).unwrap();
    let result = SearchIndex::open(tmp.path(), Some(&key()), config()).and_then(|i| {
        i.search(term("sentinelprivate"), Filters::default()).and_then(|s| s.count())
    });
    assert!(matches!(result, Err(Error::RebuildNeeded { integrity: Some(_), .. })), "{result:?}");
}

#[test]
fn plaintext_mode_manual_reload_and_cache_budget_are_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cfg = config();
    cfg.reload_policy = ReloadPolicy::Manual;
    let index = SearchIndex::create(tmp.path(), None, cfg).unwrap();
    index.apply(Batch { last_seq: 3, upserts: vec![page(1, "needle")], deletes: vec![] }).unwrap();
    assert_eq!(session(&index, term("needle")).count().unwrap(), 0);
    index.reload().unwrap();
    assert_eq!(session(&index, term("needle")).count().unwrap(), 1);
    drop(index);
    assert_eq!(
        SearchIndex::open(tmp.path(), None, config()).unwrap().last_applied_seq().unwrap(),
        3
    );
    let encrypted = tempfile::tempdir().unwrap();
    let index = create(encrypted.path(), (0..10000).map(|i| page(i, "needle archive")).collect());
    for _ in 0..3 {
        session(&index, term("needle")).page(100, None).unwrap();
    }
    assert!(index.cache_stats().unwrap().bytes <= config().cache_bytes);
    let bad = tempfile::tempdir().unwrap();
    let mut cfg = config();
    cfg.writer_memory_bytes = 1;
    assert!(matches!(SearchIndex::create(bad.path(), None, cfg), Err(Error::Input(_))));
}

#[test]
fn wrong_key_missing_segment_and_invalid_metadata_are_rebuildable() {
    let tmp = tempfile::tempdir().unwrap();
    drop(create(tmp.path(), vec![page(1, "needle")]));
    let other = vault_crypto::DataKey::from_bytes([7; 32].into());
    assert!(matches!(
        SearchIndex::open(tmp.path(), Some(&other), config()),
        Err(Error::RebuildNeeded { integrity: Some(_), .. })
    ));
    let meta = tmp.path().join("meta.json");
    let original = std::fs::read(&meta).unwrap();
    std::fs::write(&meta, b"not an encrypted index").unwrap();
    assert!(matches!(
        SearchIndex::open(tmp.path(), Some(&key()), config()),
        Err(Error::RebuildNeeded { .. })
    ));
    std::fs::write(&meta, &original).unwrap();
    let fast = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|ext| ext == "fast"))
        .unwrap();
    std::fs::remove_file(fast).unwrap();
    assert!(matches!(
        SearchIndex::open(tmp.path(), Some(&key()), config()),
        Err(Error::RebuildNeeded { .. })
    ));
}

#[test]
fn cancelled_at_commit_and_an_older_rebuild_leave_the_original_sequence() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), vec![page(1, "old")]);
    assert!(index.rebuild([Ok(page(2, "new"))], 0, |_| {}, || false).is_err());
    let checks = std::cell::Cell::new(0);
    let result = index.rebuild(
        [Ok(page(2, "new"))],
        2,
        |_| {},
        || {
            let value = checks.get() + 1;
            checks.set(value);
            value == 3
        },
    );
    assert!(matches!(result, Err(Error::Cancelled)));
    assert_eq!(index.last_applied_seq().unwrap(), 1);
    assert_eq!(ids(&session(&index, Ast::And(vec![]))), vec![1]);
}

#[test]
fn an_engine_oversize_token_cannot_be_silently_acknowledged() {
    let tmp = tempfile::tempdir().unwrap();
    let index = create(tmp.path(), vec![page(1, "old")]);
    // Tantivy otherwise silently drops a token above MAX_TOKEN_LEN. Our namespace
    // consumes one byte too. The entire batch must remain replayable on rejection.
    let oversized = "x".repeat(tantivy::tokenizer::MAX_TOKEN_LEN);
    let result = index.apply(Batch {
        last_seq: 2,
        upserts: vec![page(2, "valid"), page(3, &oversized)],
        deletes: vec![1],
    });
    assert!(matches!(result, Err(Error::Input(_))));
    assert_eq!(index.last_applied_seq().unwrap(), 1);
    assert_eq!(ids(&session(&index, Ast::And(vec![]))), vec![1]);
}

#[test]
#[cfg(unix)]
fn permission_errors_do_not_request_a_rebuild() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    drop(create(tmp.path(), vec![page(1, "old")]));
    let path = tmp.path().join("meta.json");
    let original = std::fs::metadata(&path).unwrap().permissions();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = SearchIndex::open(tmp.path(), Some(&key()), config());
    std::fs::set_permissions(&path, original).unwrap();
    assert!(matches!(result, Err(Error::Engine(_)) | Err(Error::Io(_))));
}
