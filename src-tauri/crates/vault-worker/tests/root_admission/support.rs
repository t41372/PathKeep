//! Responsibilities: isolate real owner/backend scenarios and observe complete synthetic trees.
//! Not responsible for: deciding admission or deriving expected membership from the layout table.
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};
use vault_core::{AppConfig, Vault};
#[path = "../../../../../tests/root_admission/support/tree.rs"]
pub mod tree;

pub const LEGACY: &str = include_str!("../../../../../tests/root_admission/fixtures/legacy.json");
pub const FUTURE: &str =
    include_str!("../../../../../tests/root_admission/fixtures/reader-floor.json");
pub const WRITER: &str =
    include_str!("../../../../../tests/root_admission/fixtures/writer-floor.json");
pub const PASSWORD: &str = "synthetic admission password";
static ENVIRONMENT: Mutex<()> = Mutex::new(());

pub struct Fixture {
    pub session: vault_worker::VaultSession,
    pub root: PathBuf,
    pub temp: tempfile::TempDir,
    pub config: AppConfig,
    pub vault: Option<Vault>,
    saved: Vec<(&'static str, Option<OsString>)>,
    cwd: PathBuf,
    _guard: MutexGuard<'static, ()>,
}
impl Fixture {
    pub fn new(open: bool) -> Self {
        Self::with_mode(open, false)
    }
    pub fn encrypted() -> Self {
        Self::with_mode(true, true)
    }
    fn with_mode(open: bool, encrypted: bool) -> Self {
        let guard = ENVIRONMENT.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let cwd = std::env::current_dir().unwrap();
        // Relative socket addresses avoid the Unix limit without choosing a nonstandard temp base.
        std::env::set_current_dir(temp.path()).unwrap();
        let root = PathBuf::from("root");
        fs::create_dir(&root).unwrap();
        let values = [
            ("CHB_PROJECT_ROOT", root.clone()),
            ("CHB_CHROME_USER_DATA_DIR", temp.path().join("chrome")),
            ("CHB_FIREFOX_PROFILES_DIR", temp.path().join("firefox")),
            ("CHB_SAFARI_ROOT", temp.path().join("safari")),
            ("PATHKEEP_PLATFORM_TEST_KEYRING_DIR", temp.path().join("keyring")),
            ("CHB_TEST_KEYRING_DIR", temp.path().join("keyring")),
            ("PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE", temp.path().join("key-service")),
            ("PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL", PathBuf::from("synthetic.admission.backup")),
            ("PATHKEEP_PLATFORM_TEST_LAUNCH_AGENTS_DIR", temp.path().join("agents")),
            ("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", temp.path().join("os")),
            ("PATHKEEP_TEST_SAVE_DIR", temp.path().join("saved")),
            ("PATHKEEP_TEST_BLOCK_NETWORK", PathBuf::from("1")),
        ];
        let saved = values.iter().map(|(key, _)| (*key, std::env::var_os(key))).collect();
        for (key, value) in values {
            unsafe {
                std::env::set_var(key, value);
            }
        }
        let legacy =
            if encrypted { LEGACY.replace("Plaintext", "Encrypted") } else { LEGACY.into() };
        let config: AppConfig = serde_json::from_str(&legacy).unwrap();
        let session = vault_worker::VaultSession::default();
        let vault = if open {
            fs::write(root.join("config.json"), legacy).unwrap();
            let vault = if encrypted {
                session
                    .open_password(
                        vault_core::project_paths_with_root(&root),
                        config.clone(),
                        PASSWORD.into(),
                    )
                    .unwrap()
            } else {
                session.background_vault().unwrap()
            };
            vault.search().set_paused_for_tests(true);
            Some(vault)
        } else {
            None
        };
        Self { session, root, temp, config, vault, saved, cwd, _guard: guard }
    }
    pub fn paths(&self) -> vault_core::ProjectPaths {
        vault_core::project_paths_with_root(&self.root)
    }
    pub fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    /// The data root's files, byte for byte. With the archive open, the store first finishes what
    /// it does after a write on its own: it answers a write before it empties the WAL, so a census
    /// taken just after one could see the WAL emptied while the operation under test runs, and
    /// count that as the operation's change.
    pub fn observe(&self) -> tree::Census {
        if let Some(vault) = &self.vault {
            match vault.store().pause() {
                Ok(pause) => pause.checkpoint().unwrap(),
                // A closed store writes nothing more.
                Err(vault_store::Error::Closed) => {}
                Err(error) => panic!("pausing the store: {error}"),
            }
        }
        tree::census(&self.root, vault_core::utils::sha256_hex)
    }
    pub fn outside(&self) -> tree::Census {
        // Include fixture keychain, scheduler and operation outputs as well as the data root.
        tree::census(self.temp.path(), vault_core::utils::sha256_hex)
    }
    pub fn vault(&self) -> &Vault {
        self.vault.as_ref().unwrap()
    }
    pub fn seed_install(&self) {
        self.write("staging/native-maintenance-fixture/unused", "stage sentinel");
        self.write("maintenance.bak-fixture/unused", "backup sentinel");
        self.write(
            "archive/.pk-import-journal.json",
            serde_json::to_vec(&serde_json::json!({
                "stage": self.root.join("staging/native-maintenance-fixture"),
                "backup": self.root.join("maintenance.bak-fixture"),
                "entries": [], "keep_previous": false, "committed": true
            }))
            .unwrap(),
        );
    }
    pub fn snapshot_source(&self) -> PathBuf {
        let source = self.temp.path().join("source-snapshot");
        vault_core::maintenance::snapshot(self.vault(), &source).unwrap();
        source
    }
    pub fn bundle_source(&self) -> PathBuf {
        let target = self.temp.path().join("source.pkexport");
        vault_core::migration::native::export(
            self.vault(),
            &target,
            PASSWORD,
            Default::default(),
            |_| {},
        )
        .unwrap();
        target
    }
    pub fn browser_checkpoint(&self) -> PathBuf {
        let chrome = self.temp.path().join("chrome");
        fs::create_dir_all(chrome.join("Default")).unwrap();
        fs::write(chrome.join("Last Version"), "135.0.0.0").unwrap();
        fs::write(
            chrome.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Fixture"}}}}"#,
        )
        .unwrap();
        let db = rusqlite::Connection::open(chrome.join("Default/History")).unwrap();
        db.execute_batch("CREATE TABLE urls(id INTEGER PRIMARY KEY,url TEXT,title TEXT,visit_count INTEGER,typed_count INTEGER,last_visit_time INTEGER,hidden INTEGER); CREATE TABLE visits(id INTEGER PRIMARY KEY,url INTEGER,visit_time INTEGER,from_visit INTEGER,transition INTEGER,visit_duration INTEGER,is_known_to_sync INTEGER,visited_link_id INTEGER,external_referrer_url TEXT,app_id TEXT); INSERT INTO urls VALUES(1,'https://admission.example.test/','source fixture',1,1,13412822400000000,0); INSERT INTO visits VALUES(1,1,13412822400000000,0,1,1000000,0,NULL,NULL,NULL);").unwrap();
        drop(db);
        vault_core::run_backup(self.vault(), None).unwrap();
        self.vault()
            .store()
            .read(vault_store::Db::Archive, |s| {
                s.query_one("SELECT file_path FROM snapshots LIMIT 1", (), |row| {
                    Ok(row.get::<String>(0)?)
                })
            })
            .map(PathBuf::from)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.session.close_vault();
        if let Some(vault) = self.vault.take() {
            let _ = vault.close();
        }
        for (key, value) in &self.saved {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
        std::env::set_current_dir(&self.cwd).unwrap();
    }
}

pub fn error<T>(result: Result<T, impl std::fmt::Display>) -> String {
    match result {
        Ok(_) => "SUCCESS".into(),
        Err(error) => format!("{error:#}"),
    }
}

pub fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/root_admission/fixtures")
            .join(name),
    )
    .unwrap()
}
