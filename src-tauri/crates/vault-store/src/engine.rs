//! Responsibilities: open/configure Turso on connection-owning threads and preflight keys.
//! Not responsible for: encryption implementation, queues or application migrations.
//! Failure modes: wrong keys modify files; a later invalid database permits earlier
//! writes; scratch survives reopen; IO injection is ignored; non-WAL modes are used;
//! a schema-qualified scratch pragma silently changes the main cache setting.
//! Foreign keys can silently accept orphans when the engine default is retained.
//! Tests: opening, scratch and foreign_keys tests.
use crate::{Config, Db, Error, IoLayer, Result, Session, statement_cache::StatementCache};
use std::{
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicU64},
};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

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
}
impl Settings {
    pub fn new(config: &Config, db: Db) -> Self {
        Self {
            path: config.paths.get(db).into(),
            scratch: (db != Db::Scratch).then(|| config.paths.scratch.clone()),
            archive: (db == Db::Intelligence).then(|| config.paths.archive.clone()),
            archive_key: if db == Db::Intelligence {
                config.keys[Db::Archive as usize]
            } else {
                None
            },
            key: config.keys[db as usize],
            scratch_key: config.keys[Db::Scratch as usize],
            io: config.io.clone(),
            cache_kib: config.page_cache_kib.get(),
        }
    }
    fn builder(&self, read_only: bool) -> turso::Builder {
        let mut b = turso::Builder::new_local(self.path.to_str().unwrap())
            .read_only(read_only)
            .experimental_attach(true)
            .experimental_encryption(self.key.is_some())
            .experimental_multiprocess_wal(false);
        if let Some(key) = &self.key {
            b = b.with_encryption(turso::EncryptionOpts {
                cipher: "aegis256".into(),
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

// Verify every existing file before any migration, scratch reset, directory
// creation or read/write database open. This also refuses newer schemas early.
pub(crate) fn preflight(config: &Config) -> Result<()> {
    let rt = runtime()?;
    for db in Db::ALL {
        let settings = Settings::new(config, db);
        match fs::metadata(&settings.path) {
            Ok(meta) if meta.len() > 0 => {}
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        }
        let checked =
            (|| {
                let database = rt.block_on(settings.builder(true).build())?;
                let conn = database.connect()?;
                let session = Session {
                    plans: None,
                    statements: std::cell::Cell::new(0),
                    runtime: &rt,
                    conn: &conn,
                    prepared: None,
                    read_only: true,
                    _thread: PhantomData,
                    scratch: None,
                    scratch_read: Default::default(),
                    archive: None,
                    archive_read: Default::default(),
                };
                session.query_one("SELECT count(*) FROM sqlite_schema", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })?;
                crate::migration::check_version(&session, db, &config.migrations.get(db))?;
                Ok(())
            })();
        if let Err(error) = checked {
            return Err(open_error(&settings.path, error));
        }
    }
    Ok(())
}
fn open_error(path: &Path, error: Error) -> Error {
    // An encrypted header plus NotAdb is Turso's missing-key case. Other invalid
    // headers remain Corrupt. No cryptography is performed by this inspection.
    if let Error::Corrupt { source: turso::Error::NotAdb(message) } = error {
        use std::io::Read;
        let mut header = [0; 7];
        if fs::File::open(path).and_then(|mut f| f.read_exact(&mut header)).is_ok()
            && header.starts_with(b"Turso\0")
        {
            return Error::WrongKey { source: turso::Error::NotAdb(message) };
        }
        return Error::Corrupt { source: turso::Error::NotAdb(message) };
    }
    error
}
pub(crate) fn prepare_paths(config: &Config) -> Result<()> {
    let mut canonical = Vec::new();
    for db in Db::ALL {
        let path = config.paths.get(db);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let identity = if path.exists() {
            fs::canonicalize(path)?
        } else {
            fs::canonicalize(path.parent().unwrap())?.join(path.file_name().unwrap())
        };
        if canonical.contains(&identity) {
            return Err(Error::InvalidConfig("database paths resolve to the same file".into()));
        }
        canonical.push(identity);
    }
    // Scratch is disposable, but reset only after all persistent stores verify.
    for suffix in ["", "-wal", "-shm"] {
        let mut path = config.paths.scratch.as_os_str().to_owned();
        path.push(suffix);
        match fs::remove_file(path) {
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
}
impl Engine {
    pub fn open(settings: &Settings, compiled: Arc<AtomicU64>) -> Result<Self> {
        let rt = runtime()?;
        let database = rt.block_on(settings.builder(false).build())?;
        let conn = database.connect()?;
        let engine = Self { rt, conn, database, prepared: StatementCache::new(compiled) };
        engine.configure(settings, false)?;
        Ok(engine)
    }
    pub fn reader(
        database: turso::Database,
        settings: &Settings,
        compiled: Arc<AtomicU64>,
    ) -> Result<Self> {
        let rt = runtime()?;
        let conn = database.connect()?;
        let engine = Self { rt, conn, database, prepared: StatementCache::new(compiled) };
        engine.configure(settings, true)?;
        Ok(engine)
    }
    pub fn session(&self, read_only: bool) -> Session<'_> {
        Session {
            plans: None,
            statements: std::cell::Cell::new(0),
            runtime: &self.rt,
            conn: &self.conn,
            prepared: Some(&self.prepared),
            read_only,
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
                self.conn.execute("ATTACH ?1 AS scratch", [uri.as_str()]).await?;
                // In 0.8.1 a schema-qualified cache_size still updates main.
                // Leave the attachment's native bounded page cache alone.
            }
            if let Some(path) = &settings.archive {
                let uri = scratch_uri(path, settings.archive_key.as_ref());
                self.conn.execute("ATTACH ?1 AS archive", [uri.as_str()]).await?;
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
fn scratch_uri(path: &Path, key: Option<&[u8; 32]>) -> zeroize::Zeroizing<String> {
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
        write!(uri, "?cipher=aegis256&hexkey={}", hex.as_str()).unwrap();
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
        prepare_paths(&config).unwrap();
        let _scratch =
            Engine::open(&Settings::new(&config, Db::Scratch), Default::default()).unwrap();
        let settings = Settings::new(&config, Db::Archive);
        let writer = Engine::open(&settings, Default::default()).unwrap();
        let reader =
            Engine::reader(writer.database.clone(), &settings, Default::default()).unwrap();
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
