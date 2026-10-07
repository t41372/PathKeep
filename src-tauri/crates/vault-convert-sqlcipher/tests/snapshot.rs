//! Responsibilities: preserve recovery from pre-Turso full-archive snapshots.
//! Not responsible for: browser checkpoint replay or native snapshot format tests.
//! Failure modes: corrupt/wrong-password snapshot replaces live data, old snapshot
//! is modified, missing legacy evidence is presented as recovered evidence.
use vault_core::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use vault_store::Db;

#[test]
fn legacy_full_snapshot_restores_to_turso_in_both_modes() {
    for mode in [ArchiveMode::Plaintext, ArchiveMode::Encrypted] {
        let legacy = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(legacy.path());
        let config = AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: mode.clone(),
            ..Default::default()
        };
        let password = (mode == ArchiveMode::Encrypted).then_some("snapshot password");
        vault_core::test_support::legacy_fixtures::seed_reader_pages(
            &paths,
            &config,
            password,
            &[("https://example.com/%7Earchive", "Recovered history")],
        );
        let source = paths.archive_database_path;
        let before = std::fs::read(&source).unwrap();
        let live = Vault::open_temp().unwrap();
        if password.is_some() {
            assert!(
                vault_convert_sqlcipher::restore_snapshot(
                    live.paths(),
                    Some(&live),
                    None,
                    &source,
                    Some("wrong"),
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
            Default::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(restored.vault.config().archive_mode, mode);
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
