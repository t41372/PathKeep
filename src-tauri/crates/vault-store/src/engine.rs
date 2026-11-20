//! Responsibilities: open/configure Turso on connection-owning threads and preflight keys.
//! Not responsible for: encryption implementation, queues or application migrations.
//! Failure modes: wrong keys modify files; a later invalid database permits earlier
//! writes; scratch survives reopen; IO injection is ignored; non-WAL modes are used;
//! a schema-qualified scratch pragma silently changes the main cache setting.
//! Foreign keys can silently accept orphans when the engine default is retained.
//! Tests: opening, scratch and foreign_keys tests.
use crate::{
    Config, Db, Error, IoLayer, OpenStage, Result, Session, statement_cache::StatementCache,
};
use std::{
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicU64},
};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

/// The page cipher of every encrypted database. Turso's AEGIS build (`pure-rust-crypto`,
/// which its SDK enables) picks hardware AES only when the binary is compiled for it, so
/// x86_64 builds ran software AES; compiling for it would crash on CPUs without AES-NI.
/// AES-256-GCM detects AES-NI and carry-less multiply at run time and, measured on Modal
/// (x86_64, release), matched hardware AEGIS and read 3.5 times faster than software AEGIS
/// (docs/architecture/data-model.md, "Page encryption").
const CIPHER: &str = "aes256gcm";

#[derive(Clone)]
pub(crate) struct Settings {
    pub path: PathBuf,
    pub scratch: Option<PathBuf>,
    pub archive: Option<PathBuf>,
    pub archive_key: Option<[u8; 32]>,
    pub key: Option<[u8; 32]>,
    pub scratch_key: Option<[u8; 32]>,
    pub io: Option<IoLayer>,
    pub cache_kib: usize,
    /// Where the writer and each reader say what they are running (`running.rs`).
    pub watched: Option<(Arc<crate::running::Slot>, Vec<Arc<crate::running::Slot>>)>,
}
impl Settings {
    pub fn new(config: &Config, db: Db) -> Self {
        Self {
            path: config.paths.get(db).into(),
            scratch: (db != vault_layout::scratch_database()).then(|| config.paths.scratch.clone()),
            archive: (vault_layout::database_spec(db).dependencies.contains(&Db::Archive))
                .then(|| config.paths.archive.clone()),
            archive_key: if vault_layout::database_spec(db).dependencies.contains(&Db::Archive) {
                config.key(Db::Archive)
            } else {
                None
            },
            key: config.key(db),
            scratch_key: config.key(vault_layout::scratch_database()),
            io: config.io.clone(),
            cache_kib: config.page_cache_kib.get(),
            watched: None,
        }
    }
    fn builder(&self, read_only: bool) -> turso::Builder {
        let mut b = turso::Builder::new_local(self.path.to_str().unwrap())
            .read_only(read_only)
            .experimental_attach(true)
            .experimental_encryption(true)
            .experimental_multiprocess_wal(false);
        if let Some(key) = &self.key {
            b = b.with_encryption(turso::EncryptionOpts {
                cipher: CIPHER.into(),
                // Turso owns this required SDK copy; our temporary is wiped on drop.
                hexkey: hex_key(key).to_string(),
            });
        }
        if let Some(io) = &self.io {
            b = b.with_io_impl(io.clone());
        }
        b
    }
}
pub(crate) fn runtime() -> Result<Runtime> {
    Ok(RuntimeBuilder::new_current_thread().enable_all().build()?)
}

// Verify every selected existing file before any migration, scratch reset, directory
// creation or read/write database open. This also refuses newer schemas early.
pub(crate) fn preflight(config: &Config, roles: &[Db]) -> Result<()> {
    let rt = runtime()?;
    let mut tickets = Vec::new();
    for &db in roles {
        let settings = Settings::new(config, db);
        let checked = preflight_role(&rt, config, db, &settings, |s| {
            if config.pair_seals && crate::opening::conversation_role(db) {
                tickets.push(crate::pair::ticket(s)?);
            }
            Ok(())
        });
        if let Err(error) = checked {
            let cause = open_error(&settings.path, error);
            let stage = if matches!(cause, Error::WrongKey { .. }) {
                OpenStage::KeyCheck
            } else {
                OpenStage::Preflight
            };
            return Err(cause.at_file(db, stage, &settings.path));
        }
    }
    if tickets.len() == 2 && tickets.iter().all(Option::is_some) && tickets[0] != tickets[1] {
        return Err(Error::PairUnverified.at_file(
            Db::AgentHistoryReadModel,
            OpenStage::Preflight,
            config.paths.get(Db::AgentHistoryReadModel),
        ));
    }
    Ok(())
}
fn preflight_role(
    rt: &Runtime,
    config: &Config,
    db: Db,
    settings: &Settings,
    inspect: impl FnOnce(&Session<'_>) -> Result<()>,
) -> Result<()> {
    let required = config.required_existing.contains(&db);
    match fs::metadata(&settings.path) {
        Ok(meta) if !meta.is_file() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "database path is not a regular file",
            )
            .into());
        }
        Ok(meta) if required && meta.len() < 16 => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "incomplete database header",
            )
            .into());
        }
        Ok(meta) if meta.len() == 0 => return Ok(()),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return if required { Err(Error::MissingStore(settings.path.clone())) } else { Ok(()) };
        }
        Err(e) => return Err(e.into()),
    }
    inspect_readonly(rt, settings, |session| {
        session.query_one("SELECT count(*) FROM sqlite_schema", (), |r| Ok(r.get::<i64>(0)?))?;
        crate::migration::check_version(session, db, &config.migrations.get(db))?;
        inspect(session)
    })
}

/// Inspect a damaged role without starting workers, applying migrations or attaching siblings.
impl crate::Store {
    pub fn inspect_existing<T: Send>(
        config: &Config,
        db: Db,
        inspect: impl FnOnce(&Session<'_>) -> Result<T> + Send,
    ) -> Result<T> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let rt = runtime()?;
                    let settings = Settings::new(config, db);
                    let meta = fs::metadata(&settings.path)?;
                    if !meta.is_file() || meta.len() < 16 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "incomplete database",
                        )
                        .into());
                    }
                    crate::session::catch_job(|| inspect_readonly(&rt, &settings, inspect))
                })
                .join()
                .map_err(|_| Error::JobPanicked { message: "inspecting damaged database".into() })?
        })
    }
}

fn inspect_readonly<T>(
    rt: &Runtime,
    settings: &Settings,
    inspect: impl FnOnce(&Session<'_>) -> Result<T>,
) -> Result<T> {
    let database = rt.block_on(settings.builder(true).build())?;
    let conn = database.connect()?;
    let session = Session {
        plans: None,
        watched_tables: std::cell::Cell::new(&[]),
        changes_before: std::cell::Cell::new(None),
        reader_repair: None,
        statements: std::cell::Cell::new(0),
        runtime: rt,
        conn: &conn,
        prepared: None,
        read_only: true,
        running: None,
        _thread: PhantomData,
        scratch: None,
        scratch_read: Default::default(),
        archive: None,
        archive_read: Default::default(),
    };
    inspect(&session)
}
/// Explicit retry can afford full integrity verification. No writer or migration
/// sees either candidate until the read-only pair check has completed.
pub(crate) fn verify_pair(config: &Config, full: bool, legacy_copy: bool) -> Result<bool> {
    let rt = runtime()?;
    let mut tickets = Vec::new();
    let mut legacy_roles = 0;
    for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        let settings = Settings::new(config, db);
        crate::session::catch_job(|| preflight_role(&rt,config,db,&settings,|s| {
            if full {
                let result = s.integrity_check()?;
                if result != ["ok"] { return Err(Error::InvalidConfig(format!("integrity check: {result:?}"))); }
            }
            let has_table = s.query_one("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ah_pair_seal' AND type='table')",(),|r| Ok(r.get::<i64>(0)?!=0))?;
            if !has_table { legacy_roles += 1; }
            tickets.push(if has_table { crate::pair::ticket(s)? } else {None});
            Ok(())
        })).map_err(|e| e.at_file(db,OpenStage::Recover,&settings.path))?;
    }
    if legacy_copy && legacy_roles == 2 {
        return Ok(false);
    }
    if tickets.len() != 2
        || (tickets[0] != tickets[1] && tickets.iter().all(Option::is_some))
        || (full && tickets.iter().any(Option::is_none))
    {
        return Err(Error::PairUnverified.at_file(
            Db::AgentHistoryReadModel,
            OpenStage::Recover,
            config.paths.get(Db::AgentHistoryReadModel),
        ));
    }
    Ok(true)
}

/// Read just the native header. A short, unreadable or unknown file has no established mode.
pub(crate) fn header_encrypted(path: &Path) -> std::io::Result<bool> {
    use std::io::{ErrorKind, Read};
    if !fs::metadata(path)?.is_file() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "database path is not a regular file",
        ));
    }
    let mut header = [0; 16];
    fs::File::open(path)?.read_exact(&mut header)?;
    if &header == b"SQLite format 3\0" {
        return Ok(false);
    }
    // Turso's version-zero header followed by cipher ID 2 (the AES-256-GCM mode above).
    if header.starts_with(b"Turso\0\x02") && header[7..].iter().all(|byte| *byte == 0) {
        return Ok(true);
    }
    Err(std::io::Error::new(ErrorKind::InvalidData, "unknown database header"))
}

fn open_error(path: &Path, error: Error) -> Error {
    // With a destroyed header, even the correct key can get a page decryption error.
    // With an intact encrypted header, NotAdb is Turso's missing-key failure.
    match error {
        Error::WrongKey { source }
            if header_encrypted(path)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::InvalidData) =>
        {
            Error::Corrupt { source }
        }
        Error::Corrupt { source: source @ turso::Error::NotAdb(_) }
            if matches!(header_encrypted(path), Ok(true)) =>
        {
            Error::WrongKey { source }
        }
        other => other,
    }
}
pub(crate) fn check_paths(config: &Config, roles: &[Db]) -> Result<()> {
    let mut canonical = Vec::new();
    let mut linked = Vec::new();
    for &db in roles {
        let path = config.paths.get(db);
        let identity =
            resolved_path(path).map_err(|e| e.at_file(db, OpenStage::Preflight, path))?;
        let mut alias = canonical.contains(&identity);
        canonical.push(identity);
        match turso::core::io::get_file_id(path.to_str().expect("validated database path")) {
            Ok(id) => {
                alias |= linked.contains(&id);
                linked.push(id);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::Io(e).at_file(db, OpenStage::Preflight, path)),
        }
        if alias {
            return Err(Error::InvalidConfig("database paths resolve to the same file".into())
                .at_file(db, OpenStage::Preflight, path));
        }
    }
    Ok(())
}

fn resolved_path(path: &Path) -> Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(existing) => Ok(existing),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| Error::InvalidConfig("database has no parent".into()))?;
            fs::create_dir_all(parent)?;
            Ok(fs::canonicalize(parent)?.join(
                path.file_name()
                    .ok_or_else(|| Error::InvalidConfig("database has no name".into()))?,
            ))
        }
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn reset_scratch(config: &Config) -> Result<()> {
    // Scratch is disposable, but reset only after all persistent stores verify.
    for suffix in vault_layout::database_spec(vault_layout::scratch_database())
        .companions
        .suffixes(vault_layout::CompanionUse::Reset)
    {
        let mut path = config.paths.scratch.as_os_str().to_owned();
        path.push(suffix);
        match vault_fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub(crate) struct Engine {
    pub rt: Runtime,
    pub conn: turso::Connection,
    pub database: turso::Database,
    prepared: StatementCache,
    pub running: Option<Arc<crate::running::Slot>>,
}
impl Engine {
    pub fn open(settings: &Settings, compiled: Arc<AtomicU64>) -> Result<Self> {
        let rt = runtime()?;
        let database = rt.block_on(settings.builder(false).build())?;
        let conn = database.connect()?;
        let running = settings.watched.as_ref().map(|(writer, _)| writer.clone());
        let engine = Self { rt, conn, database, prepared: StatementCache::new(compiled), running };
        engine.configure(settings, false)?;
        Ok(engine)
    }
    pub fn reader(
        database: turso::Database,
        settings: &Settings,
        compiled: Arc<AtomicU64>,
        index: usize,
    ) -> Result<Self> {
        let rt = runtime()?;
        let conn = database.connect()?;
        let running =
            settings.watched.as_ref().and_then(|(_, readers)| readers.get(index).cloned());
        let engine = Self { rt, conn, database, prepared: StatementCache::new(compiled), running };
        engine.configure(settings, true)?;
        Ok(engine)
    }
    pub fn session(&self, read_only: bool) -> Session<'_> {
        Session {
            plans: None,
            watched_tables: std::cell::Cell::new(&[]),
            changes_before: std::cell::Cell::new(None),
            reader_repair: None,
            statements: std::cell::Cell::new(0),
            runtime: &self.rt,
            conn: &self.conn,
            prepared: Some(&self.prepared),
            read_only,
            running: self.running.as_deref(),
            _thread: PhantomData,
            scratch: None,
            scratch_read: Default::default(),
            archive: None,
            archive_read: Default::default(),
        }
    }
    fn configure(&self, settings: &Settings, read_only: bool) -> Result<()> {
        self.rt.block_on(async {
            // Drain pragma result rows; execute() rejects row-returning pragmas.
            for sql in [
                "PRAGMA foreign_keys=ON".into(),
                "PRAGMA journal_mode=WAL".into(),
                "PRAGMA synchronous=FULL".into(),
                format!("PRAGMA cache_size=-{}", settings.cache_kib),
                "PRAGMA temp_store=FILE".into(),
            ] {
                let mut rows = self.conn.query(sql, ()).await?;
                while rows.next().await?.is_some() {}
            }
            if let Some(path) = &settings.scratch {
                let uri = scratch_uri(path, settings.scratch_key.as_ref());
                self.conn
                    .execute("ATTACH ?1 AS scratch", [uri.as_str()])
                    .await
                    .map_err(|e| Error::from(e).at_file(Db::Scratch, OpenStage::Open, path))?;
                // In 0.8.1 a schema-qualified cache_size still updates main.
                // Leave the attachment's native bounded page cache alone.
            }
            if let Some(path) = &settings.archive {
                let uri = scratch_uri(path, settings.archive_key.as_ref());
                self.conn
                    .execute("ATTACH ?1 AS archive", [uri.as_str()])
                    .await
                    .map_err(|e| Error::from(e).at_file(Db::Archive, OpenStage::Open, path))?;
            }
            if read_only {
                self.conn.execute("PRAGMA query_only=1", ()).await?;
            }
            Ok(())
        })
    }
}
fn hex_key(key: &[u8; 32]) -> zeroize::Zeroizing<String> {
    use std::fmt::Write;
    let mut s = zeroize::Zeroizing::new(String::with_capacity(64));
    for byte in key {
        write!(s, "{byte:02x}").unwrap();
    }
    s
}
pub(crate) fn scratch_uri(path: &Path, key: Option<&[u8; 32]>) -> zeroize::Zeroizing<String> {
    use std::fmt::Write;
    let mut uri = zeroize::Zeroizing::new(String::from("file:"));
    for byte in path.to_str().unwrap().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.:".contains(byte) {
            uri.push(*byte as char)
        } else {
            write!(uri, "%{byte:02X}").unwrap()
        }
    }
    if let Some(key) = key {
        let hex = hex_key(key);
        write!(uri, "?cipher={CIPHER}&hexkey={}", hex.as_str()).unwrap();
    }
    uri
}

impl Drop for Settings {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
        self.scratch_key.zeroize();
        self.archive_key.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_cache_and_reader_pragmas_are_applied() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::new(dir.path());
        check_paths(&config, &config.databases).unwrap();
        let _scratch =
            Engine::open(&Settings::new(&config, Db::Scratch), Default::default()).unwrap();
        let settings = Settings::new(&config, Db::Archive);
        let writer = Engine::open(&settings, Default::default()).unwrap();
        let reader =
            Engine::reader(writer.database.clone(), &settings, Default::default(), 0).unwrap();
        for engine in [&writer, &reader] {
            let main = pragma_number(engine, "PRAGMA main.cache_size");
            let scratch = pragma_number(engine, "PRAGMA scratch.cache_size");
            // Readback is connection-wide even when the pragma names a schema.
            assert_eq!((main, scratch), (-8192, -8192));
            assert_eq!(pragma_number(engine, "PRAGMA temp_store"), 1);
        }
        assert_eq!(pragma_number(&reader, "PRAGMA query_only"), 1);
    }
    fn pragma_number(engine: &Engine, sql: &str) -> i64 {
        engine.rt.block_on(async {
            let mut rows = engine.conn.query(sql, ()).await.unwrap();
            rows.next().await.unwrap().unwrap().get(0).unwrap()
        })
    }
}
