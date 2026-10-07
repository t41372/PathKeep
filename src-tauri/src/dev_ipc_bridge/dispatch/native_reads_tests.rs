//! Responsibilities: verify native archive reads through serialized desktop requests.
//! Not responsible for: ranking internals, frontend rendering or external providers.
//! Failure modes: keychain-only reads use SQLCipher, tied visits disappear between
//! pages, import review stops at its preview size, export inherits the screen cursor,
//! snapshot paths/cursors disagree with the native owner, doctor repair stays refused,
//! search or search export requires a typed password after keychain unlock.
use super::*;
use std::collections::BTreeSet;

#[test]
fn keychain_only_session_reads_imported_ties_and_exports_every_visit() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let empty = root.path().join("empty-browser-root");
    std::fs::create_dir(&empty).unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &empty);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let now = chrono::Utc::now();
    let entries=(0..41).map(|id|json!({"url":format!("https://example.com/native/{id}"),"title":format!("Native {id}"),"time_usec":now.timestamp_millis()*1000,"page_transition":"LINK"})).collect::<Vec<_>>();
    let source = root.path().join("BrowserHistory.json");
    std::fs::write(&source, serde_json::to_vec(&json!({"Browser History":entries})).unwrap())
        .unwrap();
    ready_block_on(async {
        let config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..test_config() };
        dispatch_command(
            &state,
            "initialize_archive",
            json!({"config":config,"databaseKey":"native-read-test"}),
        )
        .await
        .unwrap();
        let imported = dispatch_command(
            &state,
            "import_takeout",
            json!({"request":{"sourcePath":source,"dryRun":false}}),
        )
        .await
        .unwrap();
        assert_eq!(imported["importedItems"], 41);
        let batch = imported["importBatch"]["id"].as_i64().unwrap();
        dispatch_command(&state, "keyring_store_database_key", json!({"value":"native-read-test"}))
            .await
            .unwrap();
        dispatch_command(&state, "clear_session_database_key", json!({})).await.unwrap();
        dispatch_command(&state, "unlock_archive_from_keychain", json!({})).await.unwrap();
        assert!(state.session.get_key().unwrap().is_none());
        let snapshot = dispatch_command(&state, "app_snapshot", json!({})).await.unwrap();
        assert_eq!(snapshot["archiveStatus"]["unlocked"], true);
        assert_eq!(
            snapshot["directories"]["archiveDatabasePath"],
            snapshot["archiveStatus"]["databasePath"]
        );
        assert!(snapshot.get("recentRunsNextCursor").is_some());
        assert!(snapshot.get("recentImportBatchesNextCursor").is_some());
        let mut cursor = Value::Null;
        let mut ids = BTreeSet::new();
        loop {
            let page = dispatch_command(
                &state,
                "query_history",
                json!({"query":{"limit":4,"cursor":cursor}}),
            )
            .await
            .unwrap();
            assert_eq!(page["total"], 41);
            for row in page["items"].as_array().unwrap() {
                assert!(ids.insert(row["id"].as_i64().unwrap()));
            }
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        assert_eq!(ids.len(), 41);
        let mut cursor = Value::Null;
        let mut urls = BTreeSet::new();
        loop {
            let page = dispatch_command(
                &state,
                "preview_import_batch",
                json!({"batchId":batch,"limit":4,"cursor":cursor}),
            )
            .await
            .unwrap();
            for row in page["previewEntries"].as_array().unwrap() {
                assert!(urls.insert(row["url"].as_str().unwrap().to_owned()));
            }
            cursor = page["previewNextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        assert_eq!(urls.len(), 41);
        let batches =
            dispatch_command(&state, "load_import_batches", json!({"limit":1})).await.unwrap();
        assert_eq!(batches["items"][0]["id"], batch);
        let runs = dispatch_command(&state, "load_recent_runs", json!({"limit":1})).await.unwrap();
        assert_eq!(runs["items"][0]["runType"], "import");
        let sources = dispatch_command(&state, "load_source_stats", json!({})).await.unwrap();
        assert_eq!(sources[0]["visitCount"], 41);
        let detail = dispatch_command(
            &state,
            "get_url_detail",
            json!({"url":"https://example.com/native/0"}),
        )
        .await
        .unwrap();
        assert_eq!(detail["totalVisits"], 1);
        let day=dispatch_command(&state,"get_browse_day_insights",json!({"request":{"date":now.with_timezone(&chrono::Local).date_naive().to_string(),"profileId":null}})).await.unwrap();
        assert_eq!(day["totalPages"], 41);
        let dashboard =
            dispatch_command(&state, "load_dashboard_snapshot", json!({})).await.unwrap();
        assert_eq!(dashboard["totalVisits"], 41);
        let export = dispatch_command(
            &state,
            "export_history",
            json!({"request":{"format":"jsonl","query":{"limit":1,"page":9,"cursor":"0|0"}}}),
        )
        .await
        .unwrap();
        assert_eq!(export["count"], 41);
        assert_eq!(
            std::fs::read_to_string(export["path"].as_str().unwrap()).unwrap().lines().count(),
            41
        );
        // Search shares this exact keychain-unlocked owner with timeline reads.
        state.session.vault().unwrap().search().cycle().unwrap();
        let search =
            dispatch_command(&state, "query_history", json!({"query":{"q":"native","limit":4}}))
                .await
                .unwrap();
        assert_eq!(search["total"], 41);
        assert_eq!(search["searchState"]["coverageComplete"], true);
        let search_export = dispatch_command(
            &state,
            "export_history",
            json!({"request":{"format":"jsonl","query":{"q":"native","limit":1,"cursor":search["nextCursor"]}}}),
        ).await.unwrap();
        assert_eq!(search_export["count"], 41);
        assert_eq!(
            std::fs::read_to_string(search_export["path"].as_str().unwrap())
                .unwrap()
                .lines()
                .count(),
            41
        );
        let health = dispatch_command(&state, "doctor_report", json!({})).await.unwrap();
        assert!(
            health["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["code"] == "search-outbox" && c["ok"] == true)
        );
        dispatch_command(&state, "repair_health", json!({})).await.unwrap();
    });
    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}
