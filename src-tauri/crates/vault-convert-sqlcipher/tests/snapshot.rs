//! Responsibilities: preserve recovery from pre-Turso full-archive snapshots.
//! Not responsible for: browser checkpoint replay or native snapshot format tests.
//! Failure modes: corrupt/wrong-password snapshot replaces live data, old snapshot
//! is modified, missing legacy evidence is presented as recovered evidence, the restored
//! archive takes the snapshot's encryption instead of keeping its own.
use vault_core::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use vault_store::Db;

#[test]
fn legacy_full_snapshot_restores_to_turso_in_both_modes() {
    let live_keys = [None, Some([0x5a; 32])];
    let cases = [ArchiveMode::Plaintext, ArchiveMode::Encrypted]
        .into_iter()
        .flat_map(|mode| live_keys.map(|key| (mode.clone(), key)));
    for (mode, live_key) in cases {
        let legacy = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(legacy.path());
        let config = AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: mode.clone(),
            ..Default::default()
        };
        let password = (mode == ArchiveMode::Encrypted).then_some("snapshot password");
        vault_convert_sqlcipher::test_support::seed_reader_pages(
            &paths,
            &config,
            password,
            &[("https://example.com/%7Earchive", "Recovered history")],
        );
        let source = paths.legacy.archive;
        let before = std::fs::read(&source).unwrap();
        let live = Vault::open_temp_with_key(live_key).unwrap();
        if password.is_some() {
            assert!(
                vault_convert_sqlcipher::restore_snapshot(
                    live.paths(),
                    Some(&live),
                    None,
                    &source,
                    Some("wrong"),
                    None,
                    Default::default(),
                    |_| {},
                )
                .is_err()
            );
            assert!(live.store().is_open());
        }
        let restored = vault_convert_sqlcipher::restore_snapshot(
            live.paths(),
            Some(&live),
            None,
            &source,
            password,
            None,
            Default::default(),
            |_| {},
        )
        .unwrap();
        // The archive keeps its own encryption, whatever the snapshot's.
        let kept = if live_key.is_some() { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext };
        assert_eq!(restored.vault.config().archive_mode, kept, "a {mode:?} snapshot");
        let count = restored
            .vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(std::fs::read(&source).unwrap(), before);
        restored.vault.close().unwrap();
    }
}
