//! Responsibilities: construct a synthetic owner and browser; seed distinct durable facts.
//! Not responsible for: deriving expected coverage, mocking storage, or accessing installed data.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};
use vault_core::{AppConfig, ArchiveMode, Vault};
use vault_worker::VaultSession;

static ENVIRONMENT: Mutex<()> = Mutex::new(());
pub const PASSWORD: &str = "step-one archive password";
pub const NEW_PASSWORD: &str = "step-one changed password";
pub const EXPORT_PASSWORD: &str = "step-one independent export password";
pub const MARKERS: [&str; 17] = [
    "pkarchiveamber",
    "pkevidencebirch",
    "pkintelligencecoral",
    "pkagentdenim",
    "pkagenthistoryivory",
    "pkreadmodeljade",
    "pksearchelm",
    "pktextfuchsia",
    "pkcheckpointgold",
    "pkaskhazel",
    "discardedgeneration",
    "targetarchive",
    "targetevidence",
    "targetintelligence",
    "targetagent",
    "targetagenthistory",
    "targetreadmodel",
];

/// Holds the owner and its isolation guard until every archive handle has closed.
pub struct Fixture {
    pub session: VaultSession,
    pub root: PathBuf,
    pub sandbox: tempfile::TempDir,
    saved_env: Vec<(&'static str, Option<OsString>)>,
    _environment: MutexGuard<'static, ()>,
}

impl Fixture {
    pub fn new(encrypted: bool) -> Self {
        let guard = ENVIRONMENT.lock().unwrap_or_else(|e| e.into_inner());
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../var/vault-lifecycle");
        fs::create_dir_all(&base).unwrap();
        let sandbox = tempfile::Builder::new().prefix("scenario-").tempdir_in(base).unwrap();
        let root = sandbox.path().join("data");
        let overrides = [
            ("CHB_PROJECT_ROOT", root.clone()),
            ("CHB_AGENT_TOOLS_HOME", sandbox.path().join("no-agent-tools")),
            ("CHB_CHROME_USER_DATA_DIR", sandbox.path().join("chrome")),
            ("CHB_FIREFOX_PROFILES_DIR", sandbox.path().join("firefox")),
            ("CHB_SAFARI_ROOT", sandbox.path().join("safari")),
            ("CHB_TEST_KEYRING_DIR", sandbox.path().join("keyring")),
            ("PATHKEEP_PLATFORM_TEST_KEYRING_DIR", sandbox.path().join("keyring")),
            (
                "PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE",
                sandbox.path().join("synthetic-key-service"),
            ),
            ("PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL", sandbox.path().join("synthetic-schedule")),
            ("PATHKEEP_PLATFORM_TEST_LAUNCH_AGENTS_DIR", sandbox.path().join("launch-agents")),
            ("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", sandbox.path().join("platform")),
            ("PATHKEEP_TEST_SAVE_DIR", sandbox.path().join("saved")),
            ("PATHKEEP_TEST_BLOCK_NETWORK", PathBuf::from("1")),
        ];
        let saved_env = overrides.iter().map(|(name, _)| (*name, std::env::var_os(name))).collect();
        for (name, value) in overrides {
            // All scenarios in this test binary hold ENVIRONMENT until their owner is closed.
            unsafe {
                std::env::set_var(name, value);
            }
        }
        let session = VaultSession::default();
        let mut config = AppConfig {
            initialized: true,
            archive_mode: if encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext },
            git_enabled: false,
            checkpoint_days: 1,
            selected_profile_ids: vec!["chrome:Default".into()],
            ..Default::default()
        };
        config.ai.job_queue_paused = true;
        config.ai.content_fetch_enabled = false;
        config.ai.auto_index_after_backup = false;
        vault_worker::initialize_archive_database_with_progress(
            &session,
            &config,
            encrypted.then_some(PASSWORD),
            |_| {},
        )
        .unwrap();
        let fixture = Self { session, root, sandbox, saved_env, _environment: guard };
        fixture.vault().search().set_paused_for_tests(true);
        fixture
    }

    pub fn vault(&self) -> Vault {
        self.session.vault().unwrap()
    }

    pub fn reopen(&self, encrypted: bool) -> Vault {
        let paths = vault_core::project_paths_with_root(&self.root);
        if encrypted {
            let config = vault_core::load_config(&paths).unwrap();
            self.session.open_password(paths, config, PASSWORD.into()).unwrap()
        } else {
            self.session.background_vault().unwrap()
        }
    }

    pub fn browser(&self) -> PathBuf {
        self.sandbox.path().join("chrome/Default/History")
    }

    pub fn create_browser(&self) {
        let path = self.browser();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(self.sandbox.path().join("chrome/Last Version"), "135.0.0.0").unwrap();
        fs::write(
            self.sandbox.path().join("chrome/Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default"}}}}"#,
        )
        .unwrap();
        let source = rusqlite::Connection::open(path).unwrap();
        source.execute_batch("CREATE TABLE urls(id INTEGER PRIMARY KEY,url TEXT,title TEXT,visit_count INTEGER,typed_count INTEGER,last_visit_time INTEGER,hidden INTEGER);
            CREATE TABLE visits(id INTEGER PRIMARY KEY,url INTEGER,visit_time INTEGER,from_visit INTEGER,transition INTEGER,visit_duration INTEGER,is_known_to_sync INTEGER,visited_link_id INTEGER,external_referrer_url TEXT,app_id TEXT);
            CREATE TABLE checkpoint_marker(body TEXT);
            INSERT INTO checkpoint_marker VALUES('pkcheckpointgold');
            INSERT INTO urls VALUES(1,'https://lifecycle.example.test/page','pksearchelm',1,1,13412822400000000,0);
            INSERT INTO visits VALUES(1,1,13412822400000000,0,1,1000000,0,NULL,NULL,NULL);").unwrap();
    }

    pub fn backup(&self) -> vault_core::BackupReport {
        vault_worker::run_backup_now(&self.vault(), None, false).unwrap()
    }

    pub fn seed(&self) {
        self.create_browser();
        let backup = self.backup();
        assert_eq!(backup.run.as_ref().unwrap().new_visits, 1, "{backup:#?}");
        let vault = self.vault();
        // These are independent fixture facts, deliberately not a production role enumeration.
        for (db, marker) in [
            (vault_store::Db::Archive, "pkarchiveamber"),
            (vault_store::Db::SourceEvidence, "pkevidencebirch"),
            (vault_store::Db::Intelligence, "pkintelligencecoral"),
            (vault_store::Db::Agent, "pkagentdenim"),
            (vault_store::Db::AgentHistoryEvidence, "pkagenthistoryivory"),
            (vault_store::Db::AgentHistoryReadModel, "pkreadmodeljade"),
        ] {
            vault
                .store()
                .write(db, vault_store::Priority::Interactive, move |s| {
                    s.execute("CREATE TABLE lifecycle_marker(body TEXT NOT NULL)", ())?;
                    s.execute("INSERT INTO lifecycle_marker VALUES(?1)", [marker])?;
                    Ok(())
                })
                .unwrap();
        }
        vault_worker::save_ai_conversation(
            &vault,
            &vault_core::SaveAgentConversationRequest {
                id: "lifecycle-chat".into(),
                title: Some("Ask fixture".into()),
                provider_id: None,
                messages: vec![vault_core::AgentMessage {
                    id: "message-1".into(),
                    role: "user".into(),
                    content: "pkaskhazel".into(),
                    ..Default::default()
                }],
            },
        )
        .unwrap();
        // The offline public API cannot inject a fetched body. Seed the persisted artifact
        // and its actual metadata schema; lifecycle operations below still use real owners.
        let hash = vault_core::utils::sha256_hex(b"pktextfuchsia");
        let relative = format!("{}/{}/{}.txt", &hash[..2], &hash[2..4], hash);
        let blob = self.root.join("sidecars/intelligence-blobs").join(&relative);
        fs::create_dir_all(blob.parent().unwrap()).unwrap();
        fs::write(blob, "pktextfuchsia").unwrap();
        vault.store().write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, move |s| {
            s.execute("INSERT INTO page_content_enrichments(canonical_url,content_source,fetch_status,fetched_at,readable_text_blob_path,readable_text_bytes,text_hash,snippet_json,extraction_json,pipeline_version)
                VALUES('https://lifecycle.example.test/page','generic-readable','success','2026-11-14T00:00:00Z',?1,13,?2,'[]','{}','insights-v2')",
                vault_store::params![relative, hash])?;
            s.execute("INSERT INTO site_dictionary_overrides(target_kind,target_value,display_name,created_at,updated_at)
                VALUES('domain','lifecycle.example.test','pkintelligencecoral','2026-11-14T00:00:00Z','2026-11-14T00:00:00Z')", ())?;
            s.execute("INSERT INTO search_engine_rules(rule_id,engine_id,display_name,host_pattern,query_param_key,created_at,updated_at)
                VALUES('local-engine','local','My catalog','catalog.example.test','term','2026-11-14','2026-11-14')", ())?;
            Ok(())
        }).unwrap();
        vault.search().cycle().unwrap();
    }

    pub fn rekey(&self, mode: ArchiveMode, current: Option<&str>, next: Option<&str>) {
        vault_worker::rekey_archive_database(
            &self.session,
            &vault_worker::RekeyRequest {
                new_mode: mode,
                current_key: current.map(str::to_owned),
                new_key: next.map(str::to_owned),
                remember_in_keychain: None,
            },
        )
        .unwrap();
        self.vault().search().set_paused_for_tests(true);
    }
}

/// Returns the fixture fact through the real reader, including absence after a move.
pub fn store_marker(vault: &Vault, db: vault_store::Db) -> Option<String> {
    vault
        .store()
        .read(db, |s| {
            let exists = s.query_one(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='lifecycle_marker'",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            if exists == 0 {
                return Ok(None);
            }
            s.query_one("SELECT body FROM lifecycle_marker", (), |r| Ok(Some(r.get::<String>(0)?)))
        })
        .unwrap()
}

/// Checks index contents through its reader because segment compression hides raw strings.
pub fn search_titles(vault: &Vault) -> Vec<String> {
    let mut query = vault_core::search::SearchSession::start(
        vault,
        vault_core::search::SearchRequest { query: "pksearchelm".into(), ..Default::default() },
    )
    .unwrap();
    query
        .finish_page()
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.visit.title.unwrap_or_default())
        .collect()
}

/// Selects the single snapshot made by the fixture's worker rekey command.
pub fn safety_snapshot() -> PathBuf {
    let snapshots = vault_worker::list_recovery_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1, "fixture produced exactly one safety snapshot");
    PathBuf::from(&snapshots[0].path)
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.session.close_vault();
        for (name, value) in &self.saved_env {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}
