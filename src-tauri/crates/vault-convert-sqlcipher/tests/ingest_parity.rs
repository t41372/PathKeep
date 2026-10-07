//! Responsibilities: compare converted facts with the real area-1 browser importer.
//! Not responsible for: identical run IDs or source-provenance labels across two imports.
//! Failure modes: divergent page identities, lost visits/timestamps, different searchable
//! text/membership, native encrypted writes behaving differently from plaintext writes.
use browser_history_fixtures::{ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow};
use vault_core::{
    AppConfig, ArchiveMode, BrowserHistoryImportRequest, Vault, project_paths_with_root,
};
use vault_store::{Db, Value};

#[test]
fn conversion_matches_native_ingestion_for_the_same_browser_facts() {
    for mode in [ArchiveMode::Plaintext, ArchiveMode::Encrypted] {
        let legacy = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(legacy.path());
        let config = AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: mode.clone(),
            ..Default::default()
        };
        let password = (mode == ArchiveMode::Encrypted).then_some("parity password");
        vault_convert_sqlcipher::test_support::seed_reader_pages(
            &paths,
            &config,
            password,
            &[
                ("https://example.com/%7Ealice?utm_source=old", "Escaped Alice"),
                ("https://example.com/~alice", "Current Alice"),
            ],
        );
        let db = vault_convert_sqlcipher::test_support::open_archive(&paths, &config, password);
        let rows = db
            .prepare("SELECT id,url,title,last_visit_ms FROM urls ORDER BY id")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let mut fixture = ChromiumHistoryFixture::new();
        for (id, url, title, time) in rows {
            fixture = fixture.add_url(ChromiumUrlRow {
                id,
                url,
                title: Some(title),
                visit_count: 1,
                typed_count: 1,
                last_visit_unix_ms: time,
                hidden: false,
            });
            fixture = fixture.add_visit(ChromiumVisitRow {
                id,
                url_id: id,
                visit_time_unix_ms: time,
                from_visit: None,
                transition: None,
                visit_duration_micros: None,
                is_known_to_sync: false,
                visited_link_id: None,
                external_referrer_url: None,
                app_id: None,
            });
        }
        drop(db);
        let history = legacy.path().join("History");
        fixture.write(&history).unwrap();
        let root = tempfile::tempdir().unwrap();
        let native_paths = project_paths_with_root(root.path());
        vault_core::config::ensure_paths(&native_paths).unwrap();
        vault_core::save_config(&native_paths, &config).unwrap();
        let native = match password {
            Some(password) => {
                Vault::open_password(native_paths, config.clone(), password, None).unwrap().0
            }
            None => Vault::open(native_paths, config.clone(), None).unwrap(),
        };
        vault_core::import_browser_history(
            &native,
            &BrowserHistoryImportRequest {
                source_path: history.display().to_string(),
                dry_run: false,
                browser_family: Some("chromium".into()),
                profile_id: Some("chrome:Default".into()),
                browser_name: Some("Chrome".into()),
                profile_name: Some("Default".into()),
            },
        )
        .unwrap();
        let converted =
            vault_convert_sqlcipher::convert(&paths, &config, password, Default::default(), |_| {})
                .unwrap();
        for (sql, columns) in [
            ("SELECT url,canonical_url,registrable_domain FROM pages ORDER BY url", 3),
            (
                "SELECT u.url,v.visit_time_ms FROM visits v JOIN urls u ON u.id=v.url_id ORDER BY u.url,v.visit_time_ms",
                2,
            ),
            (
                "SELECT url,title,normalized_url,normalized_title,domain,newest_visit_ms FROM search_documents ORDER BY url",
                6,
            ),
            (
                "SELECT p.url,d.visible_visit_count,d.newest_visit_ms FROM search_document_profiles d JOIN pages p ON p.id=d.page_id ORDER BY p.url",
                3,
            ),
        ] {
            assert_eq!(
                values(&converted.vault, sql, columns),
                values(&native, sql, columns),
                "{sql}"
            );
        }
        native.close().unwrap();
        converted.vault.close().unwrap();
    }
}
fn values(vault: &Vault, sql: &'static str, columns: usize) -> Vec<Vec<Value>> {
    vault
        .store()
        .read(Db::Archive, move |s| {
            s.query(sql, (), |r| (0..columns).map(|i| r.get_value(i).map_err(Into::into)).collect())
        })
        .unwrap()
}
