//! Turso spill files hold no plaintext through `TempCryptIo`, and queries return the same
//! rows with and without it. See the failure modes in `src/temp_io.rs`.
//!
//! RERUN ON EVERY TURSO UPGRADE. The layer recognizes spill files by Turso 0.8.1's file name
//! (`tursodb_temp_file`). If a new Turso version renames it or spills through another path,
//! this test is what notices.
//!
//! How the test is built:
//! - A child process runs the workload with `TMPDIR` pointing at a directory the test owns
//!   (Turso's spill directory comes from `$TMPDIR`; setting it in a child avoids changing the
//!   environment of a multi-threaded test process).
//! - Every case runs twice on an encrypted database: with the platform IO (the positive
//!   control, which must spill plaintext, or the case proves nothing) and with `TempCryptIo`.
//! - Two independent scans: an IO layer underneath records every byte written to every file
//!   (catching plaintext that is later overwritten), and a watcher hard-links every file that
//!   appears in `$TMPDIR` or the database directory (catching files written outside the IO,
//!   such as Turso's TEMP database).
//! - Each case's rows are hashed; both runs must give the same hash.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{ChildProcess, Watcher, is_child};
use sha2::{Digest, Sha256};
use turso::core::MemoryIO;
use turso::core::io::clock::{MonotonicInstant, WallClockInstant};
use turso::core::io::{
    Buffer, Clock, Completion, File, FileId, FileSyncType, IO, OpenFlags, PlatformIO,
};
use vault_crypto::{DataKey, KeyPurpose, TempCryptIo};
use zeroize::Zeroizing;

const NEEDLES: [&[u8]; 3] = [b"PKSENTINEL", b"pksentinel.example", b"/page/"];

#[test]
fn spill_files_hold_no_plaintext_and_results_match() {
    let work = tempfile::tempdir().unwrap();
    let tmp = work.path().join("tmp");
    std::fs::create_dir(&tmp).unwrap();
    let child = ChildProcess::spawn(
        "child_spill_workload",
        &[("TMPDIR", tmp.to_str().unwrap()), ("WORKDIR", work.path().to_str().unwrap())],
    );
    for line in child.wait_success(Duration::from_secs(1800)) {
        println!("{line}");
    }
}

/// The scale needed to make every case spill. Turso's hash table spills above 32 KiB in debug
/// builds but only above 64 MiB in release builds (`vdbe/hash_table.rs`).
fn scale() -> (i64, i64) {
    if cfg!(debug_assertions) { (40_000, 60_000) } else { (250_000, 1_000_000) }
}

struct Case {
    name: &'static str,
    setup: &'static [&'static str],
    query: &'static str,
    teardown: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        name: "ORDER BY",
        setup: &[],
        query: "SELECT v.id, p.url, p.title FROM visits v JOIN pages p ON p.id = v.page_id \
                ORDER BY p.title DESC, v.id",
        teardown: &[],
    },
    Case {
        name: "GROUP BY",
        setup: &[],
        query: "SELECT p.url, count(*), max(v.time_ms) FROM visits v JOIN pages p ON p.id = v.page_id \
                GROUP BY p.url ORDER BY p.url",
        teardown: &[],
    },
    Case {
        name: "hash join",
        setup: &[],
        query: "SELECT count(*), sum(length(a.k)) FROM \
                (SELECT p.url || '#' || v.id AS k FROM visits v JOIN pages p ON p.id = v.page_id) a \
                JOIN (SELECT p.url || '#' || v.id AS k FROM visits v JOIN pages p ON p.id = v.page_id) b \
                ON a.k = b.k",
        teardown: &[],
    },
    Case {
        name: "CREATE INDEX",
        setup: &["CREATE INDEX pages_title_url ON pages(title, url)"],
        query: "SELECT title, url FROM pages \
                WHERE title >= 'PKSENTINEL title m' ORDER BY title, url LIMIT 200",
        teardown: &["DROP INDEX pages_title_url"],
    },
    Case {
        name: "scratch ATTACH",
        setup: &["CREATE TABLE scratch.work AS SELECT p.url, p.title, v.time_ms \
             FROM visits v JOIN pages p ON p.id = v.page_id"],
        query: "SELECT url, count(*), min(title) FROM scratch.work GROUP BY url ORDER BY count(*) DESC, url",
        teardown: &["DROP TABLE scratch.work"],
    },
];

/// Child: builds the database, runs every case under both IO layers and checks the results.
#[test]
#[ignore = "runs only as a child of spill_files_hold_no_plaintext_and_results_match"]
fn child_spill_workload() {
    if !is_child() {
        return;
    }
    let work = PathBuf::from(std::env::var("WORKDIR").unwrap());
    let tmp = PathBuf::from(std::env::var("TMPDIR").unwrap());
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(run_workload(&work, &tmp));
}

async fn run_workload(work: &Path, tmp: &Path) {
    let data_key = DataKey::from_bytes(Zeroizing::new([7u8; 32]));
    let db_dir = work.join("db");
    std::fs::create_dir(&db_dir).unwrap();
    let (pages, visits) = scale();
    build_archive(&db_dir, &data_key, pages, visits).await;
    println!("archive: {pages} pages, {visits} visits, load {}", common::load_averages());

    let mut failures = Vec::new();
    for case in CASES {
        let plain = run_case(case, &db_dir, tmp, work, &data_key, false).await;
        let crypt = run_case(case, &db_dir, tmp, work, &data_key, true).await;
        println!(
            "{:<15} platform IO: {} spill files, {:>9} bytes, plaintext {:<3} {:>7.0} ms | \
             TempCryptIo: {} spill files, {:>9} bytes, plaintext {:<3} {:>7.0} ms | rows {} {}",
            case.name,
            plain.spill_files,
            plain.spill_bytes,
            yes_no(!plain.plaintext.is_empty()),
            plain.ms,
            crypt.spill_files,
            crypt.spill_bytes,
            yes_no(!crypt.plaintext.is_empty()),
            crypt.ms,
            crypt.rows,
            if plain.digest == crypt.digest { "identical" } else { "DIFFERENT" },
        );
        if plain.spill_files == 0 || plain.plaintext.is_empty() {
            failures.push(format!("{}: the positive control did not spill plaintext", case.name));
        }
        if crypt.spill_files == 0 || crypt.encrypted_files == 0 {
            failures.push(format!("{}: TempCryptIo saw no spill file", case.name));
        }
        if !crypt.plaintext.is_empty() {
            failures.push(format!("{}: plaintext on disk: {:?}", case.name, crypt.plaintext));
        }
        if plain.digest != crypt.digest || plain.rows != crypt.rows {
            failures.push(format!("{}: results differ", case.name));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

struct CaseResult {
    rows: u64,
    digest: [u8; 32],
    ms: f64,
    spill_files: usize,
    spill_bytes: u64,
    encrypted_files: u64,
    /// "<file>: <needle>" for every needle found, by either scan.
    plaintext: Vec<String>,
}

async fn run_case(
    case: &Case,
    db_dir: &Path,
    tmp: &Path,
    work: &Path,
    data_key: &DataKey,
    encrypt_spills: bool,
) -> CaseResult {
    let recorder = RecordingIo::new();
    let crypt = encrypt_spills.then(|| TempCryptIo::wrapping(recorder.clone(), data_key));
    let io: Arc<dyn IO> = match &crypt {
        Some(crypt) => crypt.clone(),
        None => recorder.clone(),
    };
    let (_db, conn) = open(&db_dir.join("archive.db"), data_key, Some(io)).await;
    // A small page cache makes the sorter spill: its in-memory budget is `cache_size`.
    drain(&conn, "PRAGMA cache_size=-256").await;
    drain(&conn, "PRAGMA temp_store=FILE").await;
    let scratch_key = data_key.derive(KeyPurpose::ScratchDatabase).to_hex();
    let scratch_uri = format!(
        "file:{}?cipher=aegis256&hexkey={}",
        db_dir.join("scratch.db").display(),
        *scratch_key
    );
    conn.execute(&format!("ATTACH '{scratch_uri}' AS scratch"), ()).await.unwrap();

    let quarantine =
        work.join(format!("quarantine-{}-{}", case.name.replace(' ', "_"), encrypt_spills));
    let watcher = Watcher::start(vec![tmp.to_path_buf(), db_dir.to_path_buf()], quarantine);
    let started = Instant::now();
    for statement in case.setup {
        conn.execute(*statement, ()).await.unwrap();
    }
    let (rows, digest) = digest_rows(&conn, case.query).await;
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    for statement in case.teardown {
        conn.execute(*statement, ()).await.unwrap();
    }
    conn.execute("DETACH scratch", ()).await.unwrap();
    drain(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").await;
    drop(conn);

    let watched = watcher.finish(&NEEDLES);
    let mut plaintext: Vec<String> = Vec::new();
    for (file, needles) in &watched {
        for needle in needles {
            plaintext.push(format!("{} (final bytes): {needle}", file.path.display()));
        }
    }
    for (path, needle) in recorder.plaintext_writes() {
        plaintext.push(format!("{path} (write): {needle}"));
    }
    let spills: Vec<_> = watched.iter().filter(|(f, _)| f.path.starts_with(tmp)).collect();
    CaseResult {
        rows,
        digest,
        ms,
        spill_files: spills.len(),
        spill_bytes: spills.iter().map(|(f, _)| f.peak_bytes).sum(),
        encrypted_files: crypt.as_ref().map_or(0, |c| c.files_encrypted()),
        plaintext,
    }
}

async fn build_archive(db_dir: &Path, data_key: &DataKey, pages: i64, visits: i64) {
    let (_db, conn) = open(&db_dir.join("archive.db"), data_key, None).await;
    conn.execute(
        "CREATE TABLE pages(id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL)",
        (),
    )
    .await
    .unwrap();
    conn.execute("CREATE TABLE visits(id INTEGER PRIMARY KEY, page_id INTEGER NOT NULL, time_ms INTEGER NOT NULL)", ())
        .await
        .unwrap();
    let words = ["alpha", "harbor", "kernel", "meadow", "quartz", "river", "solstice", "zephyr"];
    for start in (1..=pages).step_by(1000) {
        let values: Vec<String> = (start..(start + 1000).min(pages + 1))
            .map(|i| {
                let word = words[(i as usize * 7) % words.len()];
                format!("({i}, 'https://pksentinel.example/page/{i}?ref={word}', 'PKSENTINEL title {word} {i}')")
            })
            .collect();
        conn.execute(&format!("INSERT INTO pages VALUES {}", values.join(",")), ()).await.unwrap();
    }
    for start in (1..=visits).step_by(2000) {
        let values: Vec<String> = (start..(start + 2000).min(visits + 1))
            .map(|i| format!("({i}, {}, {})", (i * 7919) % pages + 1, 1_700_000_000_000 + i * 1000))
            .collect();
        conn.execute(&format!("INSERT INTO visits VALUES {}", values.join(",")), ()).await.unwrap();
    }
    drain(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").await;
}

async fn open(
    path: &Path,
    data_key: &DataKey,
    io: Option<Arc<dyn IO>>,
) -> (turso::Database, turso::Connection) {
    let mut builder = turso::Builder::new_local(path.to_str().unwrap())
        .experimental_encryption(true)
        .experimental_attach(true)
        .with_encryption(turso::EncryptionOpts {
            cipher: "aegis256".into(),
            hexkey: data_key.derive(KeyPurpose::ArchiveDatabase).to_hex().to_string(),
        });
    if let Some(io) = io {
        builder = builder.with_io_impl(io);
    }
    let db = builder.build().await.unwrap();
    let conn = db.connect().unwrap();
    (db, conn)
}

async fn drain(conn: &turso::Connection, sql: &str) {
    let mut rows = conn.query(sql, ()).await.unwrap();
    while rows.next().await.unwrap().is_some() {}
}

async fn digest_rows(conn: &turso::Connection, sql: &str) -> (u64, [u8; 32]) {
    let mut rows = conn.query(sql, ()).await.unwrap();
    let mut hasher = Sha256::new();
    let mut count = 0;
    while let Some(row) = rows.next().await.unwrap() {
        for i in 0..row.column_count() {
            hasher.update(format!("{:?}\u{1f}", row.get_value(i).unwrap()).as_bytes());
        }
        hasher.update(b"\x1e");
        count += 1;
    }
    (count, hasher.finalize().into())
}

/// Platform IO that scans every buffer written to any file for the needles, before it reaches
/// the disk. Sits underneath `TempCryptIo`, so it sees exactly the bytes the disk would.
struct RecordingIo {
    inner: Arc<dyn IO>,
    hits: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
}

impl RecordingIo {
    fn new() -> Arc<RecordingIo> {
        Arc::new(RecordingIo { inner: Arc::new(PlatformIO::new().unwrap()), hits: Arc::default() })
    }

    fn plaintext_writes(&self) -> Vec<(String, String)> {
        let hits = self.hits.lock().unwrap();
        hits.iter()
            .flat_map(|(path, needles)| needles.iter().map(move |n| (path.clone(), n.clone())))
            .collect()
    }
}

struct RecordingFile {
    inner: Arc<dyn File>,
    path: String,
    hits: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
}

impl RecordingFile {
    fn record(&self, buffer: &Buffer) {
        let data = buffer.as_slice();
        for needle in NEEDLES {
            if common::contains(data, needle) {
                let needle = String::from_utf8_lossy(needle).into_owned();
                let mut hits = self.hits.lock().unwrap();
                let list = hits.entry(self.path.clone()).or_default();
                if !list.contains(&needle) {
                    list.push(needle);
                }
            }
        }
    }
}

impl File for RecordingFile {
    fn lock_file(&self, exclusive: bool) -> turso::core::Result<()> {
        self.inner.lock_file(exclusive)
    }
    fn unlock_file(&self) -> turso::core::Result<()> {
        self.inner.unlock_file()
    }
    fn pread(&self, pos: u64, c: Completion) -> turso::core::Result<Completion> {
        self.inner.pread(pos, c)
    }
    fn pwrite(
        &self,
        pos: u64,
        buffer: Arc<Buffer>,
        c: Completion,
    ) -> turso::core::Result<Completion> {
        self.record(&buffer);
        self.inner.pwrite(pos, buffer, c)
    }
    fn pwritev(
        &self,
        pos: u64,
        buffers: Vec<Arc<Buffer>>,
        c: Completion,
    ) -> turso::core::Result<Completion> {
        for buffer in &buffers {
            self.record(buffer);
        }
        self.inner.pwritev(pos, buffers, c)
    }
    fn sync(&self, c: Completion, sync_type: FileSyncType) -> turso::core::Result<Completion> {
        self.inner.sync(c, sync_type)
    }
    fn size(&self) -> turso::core::Result<u64> {
        self.inner.size()
    }
    fn truncate(&self, len: u64, c: Completion) -> turso::core::Result<Completion> {
        self.inner.truncate(len, c)
    }
}

impl Clock for RecordingIo {
    fn current_time_monotonic(&self) -> MonotonicInstant {
        self.inner.current_time_monotonic()
    }
    fn current_time_wall_clock(&self) -> WallClockInstant {
        self.inner.current_time_wall_clock()
    }
}

impl IO for RecordingIo {
    fn open_file(
        &self,
        path: &str,
        flags: OpenFlags,
        direct: bool,
    ) -> turso::core::Result<Arc<dyn File>> {
        let inner = self.inner.open_file(path, flags, direct)?;
        Ok(Arc::new(RecordingFile { inner, path: path.to_owned(), hits: self.hits.clone() }))
    }
    fn open_shared_wal_file(&self, path: &str) -> turso::core::Result<Arc<dyn File>> {
        self.inner.open_shared_wal_file(path)
    }
    fn remove_file(&self, path: &str) -> turso::core::Result<()> {
        self.inner.remove_file(path)
    }
    fn supports_shared_wal_coordination(&self) -> bool {
        self.inner.supports_shared_wal_coordination()
    }
    fn step(&self) -> turso::core::Result<()> {
        self.inner.step()
    }
    fn cancel(&self, c: &[Completion]) -> turso::core::Result<()> {
        self.inner.cancel(c)
    }
    fn drain_completions(&self, c: &[Completion]) -> turso::core::Result<()> {
        self.inner.drain_completions(c)
    }
    fn wait_for_completion(&self, c: Completion) -> turso::core::Result<()> {
        self.inner.wait_for_completion(c)
    }
    fn generate_random_number(&self) -> i64 {
        self.inner.generate_random_number()
    }
    fn fill_bytes(&self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }
    fn get_memory_io(&self) -> Arc<MemoryIO> {
        self.inner.get_memory_io()
    }
    fn register_fixed_buffer(&self, ptr: NonNull<u8>, len: usize) -> turso::core::Result<u32> {
        self.inner.register_fixed_buffer(ptr, len)
    }
    fn yield_now(&self) {
        self.inner.yield_now()
    }
    fn sleep(&self, duration: Duration) {
        self.inner.sleep(duration)
    }
    fn file_id(&self, path: &str) -> turso::core::Result<FileId> {
        self.inner.file_id(path)
    }
}
