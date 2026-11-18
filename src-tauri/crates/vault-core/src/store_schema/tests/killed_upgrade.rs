//! Responsibilities: prove that killing the app while it updates an archive v0.4.0 wrote leaves
//! one the next launch finishes updating, row for row as an update nobody stopped leaves it.
//! Not responsible for: power loss (what reaches the disk), the conversion from the old SQLCipher
//! format (foreground work with its own tests), or Turso's recovery in general (ADR-012).
//!
//! Nothing asks before PathKeep quits while the update runs: the process ends and the Store
//! worker stops wherever it was, as under `kill -9`. So a child process runs the real migrations
//! on a real version 1 archive and is killed with SIGKILL. It is not killed after a guessed
//! delay: it holds still at a chosen point and announces it, so every run kills at the same place:
//! - inside a backfill batch, its rows written and the transaction not committed (Archive
//!   version 15 links visits to their openers; Intelligence version 2 moves embeddings, a batch
//!   large enough that much of it is already in the log on disk);
//! - just after a batch committed, before the next one starts;
//! - inside Intelligence version 2's setup, its tables renamed and the version not recorded.
//!
//! One archive goes through all of them in turn, each launch carrying on from the last kill, and
//! a final open must finish where the last kill left off, not start over. A kill in the middle of
//! one long statement (an index half built on a large archive) has no point to hold at: the
//! kills inside a transaction stand for it in the gate, and `killed_at_moments_nobody_chose`, a
//! campaign run by hand, kills a large archive's update at spread-out moments to show it.
//!
//! Failure modes: a kill rolls back less or more than its transaction; a batch commits without
//! its resume cursor, so the next launch skips or repeats rows; setup commits without its version
//! marker, or the reverse; a resumed update starts from the first batch; a killed process leaves
//! the archive locked, unreadable or failing `integrity_check`; any of it only with a key.
use super::config;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use vault_store::{Db, Error, Migration, Priority, Session, Store};

const VISITS: i64 = 100_001;
const EMBEDDINGS: i64 = 100_001;
const URLS: i64 = 1_000;
const KEY: [u8; 32] = [0x5a; 32];

const CHILD: &str = "store_schema::tests::killed_upgrade::killed_upgrade_child";
const ROOT_VAR: &str = "PATHKEEP_KILLED_UPGRADE_ROOT";
const KEY_VAR: &str = "PATHKEEP_KILLED_UPGRADE_KEY";
const STOP_VAR: &str = "PATHKEEP_KILLED_UPGRADE_STOP";
const SAY: &str = "killed-upgrade:";

#[test]
fn a_killed_upgrade_of_a_plain_archive_finishes_at_the_next_launch() {
    killed_upgrade(None);
}

#[test]
fn a_killed_upgrade_of_an_encrypted_archive_finishes_at_the_next_launch() {
    killed_upgrade(Some(KEY));
}

fn killed_upgrade(key: Option<[u8; 32]>) {
    let seed = tempfile::tempdir().unwrap();
    seed_version_one(seed.path(), key, VISITS, EMBEDDINGS);

    let reference = tempfile::tempdir().unwrap();
    copy_dir(seed.path(), reference.path());
    let (events, store) = open_and_finish(reference.path(), key);
    // Batches of 50,000 rows: each backfill has a batch before and after the kills.
    assert_eq!(first(&events, "Archive"), Some(50_000), "{events:?}");
    assert_eq!(first(&events, "Intelligence"), Some(0), "{events:?}");
    assert_links(&store, VISITS);
    let expected = contents(&store);
    drop(store);

    let killed = tempfile::tempdir().unwrap();
    copy_dir(seed.path(), killed.path());
    let root = killed.path();

    // Linking batch 2 written, not committed.
    let events = run_until_killed(root, key, "Archive inside 100000");
    assert_eq!(events, ["Archive 50000"]);
    // Batch 2 again, not batch 1; killed once it committed.
    let events = run_until_killed(root, key, "Archive after 100000");
    assert_eq!(events, ["Archive 100000"]);
    // The last visit linked, then Intelligence's tables renamed, its version not recorded.
    let events = run_until_killed(root, key, "Intelligence setup");
    assert_eq!(events, ["Archive 100001", "Archive 100001"]);
    // Setup again; killed once the first move committed.
    let events = run_until_killed(root, key, "Intelligence after 50000");
    assert_eq!(events, ["Intelligence 0", "Intelligence 50000"]);
    // Move 2 written, not committed: copied into the new table and deleted from the old one.
    // Too large for the page cache, much of it is in the log on disk when the process dies, so
    // this kill leaves a transaction for recovery to throw away, not only memory.
    let wal = root.join("derived/intelligence.db-wal");
    let events = run_until_killed_with(root, key, "Intelligence inside 100000", || {
        let written = std::fs::metadata(&wal).map_or(0, |m| m.len());
        assert!(written > 0, "nothing of the uncommitted move reached {}", wal.display());
    });
    assert_eq!(events, Vec::<String>::new());

    let (events, store) = open_and_finish(root, key);
    assert_eq!(first(&events, "Archive"), None, "{events:?}");
    assert_eq!(first(&events, "Intelligence"), Some(100_000), "{events:?}");
    assert_eq!(contents(&store), expected);
    for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
        assert_eq!(store.read(db, |s| s.integrity_check()).unwrap(), ["ok"], "{db:?}");
    }
    drop(store);
    let (events, store) = open_and_finish(root, key);
    assert_eq!(events, Vec::<(String, u64)>::new());
    drop(store);
}

/// Kills at moments nobody chose, spread evenly over the time a large archive's update takes to
/// commit its first linking batch: Archive versions 2 to 14, most of it spent building or dropping
/// indexes over every visit, then version 15's first batch. Each kill is classed by the version
/// the archive had reached (the next one was running), then the update is finished and compared
/// with one nobody stopped. Where a kill lands depends on timing, so this is run by hand, not in
/// the gate (`PATHKEEP_KILL_CAMPAIGN_KEY=1` encrypts the archive):
///
/// ```text
/// PATHKEEP_KILL_CAMPAIGN_VISITS=1000000 PATHKEEP_KILL_CAMPAIGN_KILLS=20 cargo test \
///   -p vault-core --lib killed_at_moments_nobody_chose -- --ignored --nocapture
/// ```
#[test]
#[ignore = "explicit campaign: where its kills land depends on timing"]
fn killed_at_moments_nobody_chose() {
    let setting = |name: &str, default: i64| {
        std::env::var(name).map_or(default, |value| value.parse().unwrap())
    };
    let visits = setting("PATHKEEP_KILL_CAMPAIGN_VISITS", 1_000_000);
    let kills = setting("PATHKEEP_KILL_CAMPAIGN_KILLS", 20);
    let key = (setting("PATHKEEP_KILL_CAMPAIGN_KEY", 0) == 1).then_some(KEY);
    let seed = tempfile::tempdir().unwrap();
    seed_version_one(seed.path(), key, visits, 1_000);

    let reference = tempfile::tempdir().unwrap();
    copy_dir(seed.path(), reference.path());
    let (_, store) = open_and_finish(reference.path(), key);
    assert_links(&store, visits);
    let expected = contents(&store);
    drop(store);

    let probe = tempfile::tempdir().unwrap();
    copy_dir(seed.path(), probe.path());
    let (mut child, rx) = start_child(probe.path(), key, "none");
    let started = Instant::now();
    loop {
        let line = rx.recv_timeout(Duration::from_secs(1_800)).unwrap();
        if line.starts_with("event Archive") {
            break;
        }
    }
    let window = started.elapsed();
    child.kill().unwrap();
    child.wait().unwrap();
    drop(probe);

    let mut landed = std::collections::BTreeMap::<i64, usize>::new();
    for kill in 0..kills {
        let delay = window.mul_f64((kill as f64 + 0.5) / kills as f64);
        let root = tempfile::tempdir().unwrap();
        copy_dir(seed.path(), root.path());
        let (mut child, rx) = start_child(root.path(), key, "none");
        let deadline = Instant::now() + delay;
        let mut said = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(line) => said.push(line),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(error) => panic!("kill {kill}: the child ended first: {error}, {said:?}"),
            }
        }
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        assert!(!said.iter().any(|line| line == "finished"), "kill {kill}: {said:?}");
        let version = archive_version(root.path(), key);
        eprintln!(
            "kill {kill}: {delay:.2?} into {window:.2?}, Archive at version {version}, {said:?}"
        );
        *landed.entry(version).or_default() += 1;
        let (_, store) = open_and_finish(root.path(), key);
        assert_eq!(contents(&store), expected, "kill {kill}");
        for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
            assert_eq!(store.read(db, |s| s.integrity_check()).unwrap(), ["ok"], "{db:?}");
        }
    }
    eprintln!("{visits} visits, {kills} kills by the Archive version they found: {landed:?}");
}

/// The Archive version a killed update left, read without updating anything: an app that knows
/// only version 1 is told the version it found.
fn archive_version(root: &Path, key: Option<[u8; 32]>) -> i64 {
    let mut cfg = config(root, key);
    cfg.migrations.register(Db::Archive, vec![super::super::archive::migration()]);
    cfg.migrations.register(Db::Intelligence, vec![super::super::intelligence::migration()]);
    match Store::open(cfg) {
        Err(Error::SchemaTooNew { db: Db::Archive, found, .. }) => found,
        Ok(store) => {
            drop(store);
            1
        }
        Err(error) => panic!("{error}"),
    }
}

/// The point a child holds still at, from `STOP_VAR`: `<database> inside <processed>` stops in
/// the batch that brings the count to `processed`, before it commits; `<database> after
/// <processed>` stops once that batch committed; `Intelligence setup` stops in version 2's setup.
#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Inside(Db, u64),
    After(Db, u64),
    IntelligenceSetup,
    /// `none`: runs the update to the end unless killed first.
    Never,
}

fn parse(stop: &str) -> Stop {
    if stop == "none" {
        return Stop::Never;
    }
    let words: Vec<&str> = stop.split(' ').collect();
    let db = match words[0] {
        "Archive" => Db::Archive,
        "Intelligence" => Db::Intelligence,
        other => panic!("no database {other}"),
    };
    match words[1..] {
        ["setup"] if db == Db::Intelligence => Stop::IntelligenceSetup,
        ["inside", n] => Stop::Inside(db, n.parse().unwrap()),
        ["after", n] => Stop::After(db, n.parse().unwrap()),
        _ => panic!("no stop {stop}"),
    }
}

#[test]
#[ignore = "child process the killed_upgrade tests start and kill"]
fn killed_upgrade_child() {
    let Some(root) = std::env::var_os(ROOT_VAR) else { return };
    let root = PathBuf::from(root);
    let key = (std::env::var(KEY_VAR).unwrap() == "1").then_some(KEY);
    let stop = parse(&std::env::var(STOP_VAR).unwrap());
    // Captured when the registry is built, so it is registered first. It runs on the Store's
    // migration thread right after a batch commits, before the next one starts.
    crate::register_archive_upgrade_listener(move |event| {
        let Some(database) = event.database.as_deref() else { return };
        say(&format!("event {database} {}", event.processed));
        let db = if database == "Archive" { Db::Archive } else { Db::Intelligence };
        if stop == Stop::After(db, event.processed) {
            hold();
        }
    });
    let mut cfg = config(&root, key);
    match stop {
        Stop::Inside(Db::Archive, n) => {
            let mut steps = vec![super::super::archive::migration()];
            steps.extend(super::super::backup_identity::archive());
            // Versions only ever go on the end: version 15 stays the fifteenth step.
            steps[14] = steps[14].clone().with_backfill(
                super::super::backup_identity::legacy_openers_pending,
                move |s| {
                    let progress = super::super::backup_identity::link_legacy_openers(s)?;
                    hold_inside(progress.as_ref(), n);
                    Ok(progress)
                },
            );
            cfg.migrations.register(Db::Archive, steps);
        }
        Stop::Inside(_, n) => {
            let setup = super::super::intelligence_lineage::migration().with_backfill(
                super::super::intelligence_lineage::pending,
                move |s| {
                    let progress = super::super::intelligence_lineage::backfill(s)?;
                    hold_inside(progress.as_ref(), n);
                    Ok(progress)
                },
            );
            cfg.migrations
                .register(Db::Intelligence, vec![super::super::intelligence::migration(), setup]);
        }
        Stop::IntelligenceSetup => {
            let setup = Migration::rust(2, |s: &Session<'_>| {
                super::super::intelligence_lineage::apply(s)?;
                hold()
            });
            cfg.migrations
                .register(Db::Intelligence, vec![super::super::intelligence::migration(), setup]);
        }
        Stop::After(..) | Stop::Never => {}
    }
    let store = Store::open(cfg).unwrap();
    say("finished");
    drop(store);
}

fn hold_inside(progress: Option<&vault_store::BackfillProgress>, processed: u64) {
    if progress.is_some_and(|p| p.processed == processed && !p.complete) {
        hold();
    }
}

/// Announces the point and waits to be killed; gives up if nobody does, so a test that died
/// first leaves no process behind.
fn hold() -> ! {
    say("holding");
    std::thread::sleep(Duration::from_secs(600));
    std::process::exit(3);
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{SAY} {line}").unwrap();
    out.flush().unwrap();
}

/// Starts a child at `stop`, kills it there, and returns the progress it reported first.
fn run_until_killed(root: &Path, key: Option<[u8; 32]>, stop: &str) -> Vec<String> {
    run_until_killed_with(root, key, stop, || {})
}

/// [`run_until_killed`], running `while_held` while the child holds still, before the kill.
fn run_until_killed_with(
    root: &Path,
    key: Option<[u8; 32]>,
    stop: &str,
    while_held: impl FnOnce(),
) -> Vec<String> {
    let (mut child, rx) = start_child(root, key, stop);
    let mut events = Vec::new();
    let outcome = loop {
        match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(line) if line == "holding" => break Ok(()),
            Ok(line) => match line.strip_prefix("event ") {
                Some(event) => events.push(event.to_owned()),
                None => break Err(format!("the update got past {stop}: {line}")),
            },
            Err(error) => break Err(format!("never reached {stop}: {error}")),
        }
    };
    let held = outcome.map(|()| std::panic::catch_unwind(std::panic::AssertUnwindSafe(while_held)));
    child.kill().unwrap();
    let status = child.wait().unwrap();
    match held {
        Err(message) => panic!("{message}; reported {events:?}"),
        Ok(Err(panic)) => std::panic::resume_unwind(panic),
        Ok(Ok(())) => {}
    }
    assert!(!status.success(), "{stop}: {status}");
    events
}

/// A child updating the archive at `root`, and what it says, line by line.
fn start_child(root: &Path, key: Option<[u8; 32]>, stop: &str) -> (Child, mpsc::Receiver<String>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([CHILD, "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(ROOT_VAR, root)
        .env(KEY_VAR, if key.is_some() { "1" } else { "0" })
        .env(STOP_VAR, stop)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            // libtest may have started the line with the test's name.
            if let Some((_, said)) = line.split_once(SAY) {
                if tx.send(said.trim().to_owned()).is_err() {
                    break;
                }
            }
        }
    });
    (child, rx)
}

/// Opens with every migration, as a launch does, and keeps the update's progress reports.
fn open_and_finish(root: &Path, key: Option<[u8; 32]>) -> (Vec<(String, u64)>, Store) {
    let root = root.to_owned();
    let mut events = Vec::new();
    let store = crate::with_archive_upgrade_progress(
        |event| {
            if let Some(database) = event.database {
                events.push((database, event.processed));
            }
        },
        // The registry captures the progress reporter where it is built: on the update's thread.
        move || Ok(Store::open(config(&root, key))?),
    )
    .unwrap();
    (events, store)
}

fn first(events: &[(String, u64)], database: &str) -> Option<u64> {
    events.iter().find(|(db, _)| db == database).map(|(_, processed)| *processed)
}

/// What a browser row id named as the opener; most link within the profile, some cannot.
fn opener(visit: i64, visits: i64) -> Option<i64> {
    match visit % 10 {
        0 => None,
        1 => Some(0),
        2 => Some(90_000_000 + visit),
        3 if visit + 60_000 <= visits => Some(visit + 60_000),
        4 => Some(visit),
        5 if visit > 60_000 => Some(visit - 60_000),
        6 => Some(visit - 1),
        _ if visit > 2 => Some(visit - 2),
        _ => None,
    }
}

/// Visit n is browser row n of profile 1 + n % 2, so an opener links when it is another row of
/// the same profile that the archive holds.
fn parent(visit: i64, visits: i64) -> Option<i64> {
    opener(visit, visits).filter(|&o| o > 0 && o <= visits && o != visit && o % 2 == visit % 2)
}

/// Archive version 15 must have linked exactly the visits whose opener it holds.
fn assert_links(store: &Store, visits: i64) {
    let (count, sum) = (1..=visits)
        .filter_map(|visit| parent(visit, visits))
        .fold((0_i64, 0_i64), |(count, sum), parent| (count + 1, sum + parent));
    assert!(count > visits / 3, "{count}");
    let found = store
        .read(Db::Archive, |s| {
            s.query_one(
                "SELECT COUNT(*),SUM(parent_visit_id) FROM visits WHERE parent_visit_id IS NOT NULL",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
            )
        })
        .unwrap();
    assert_eq!(found, (count, sum));
}

/// An archive as v0.4.0 left it: Archive and Intelligence at version 1.
fn seed_version_one(root: &Path, key: Option<[u8; 32]>, visits: i64, embeddings: i64) {
    let mut cfg = config(root, key);
    cfg.migrations.register(Db::Archive, vec![super::super::archive::migration()]);
    cfg.migrations.register(Db::Intelligence, vec![super::super::intelligence::migration()]);
    let store = Store::open(cfg).unwrap();
    store
        .write(Db::Archive, Priority::Batch, |s| {
            s.execute(
                "INSERT INTO runs(id,run_type,trigger,started_at,status)
                 VALUES(1,'backup','scheduled','2026-10-01T00:00:00Z','completed')",
                (),
            )?;
            s.execute(
                "INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at)
                 VALUES(1,'chrome','Default','/profiles/1','2026-10-01T00:00:00Z'),
                       (2,'chrome','Profile 1','/profiles/2','2026-10-01T00:00:00Z')",
                (),
            )?;
            let mut page = s.prepare(
                "INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3)",
            )?;
            let mut url = s.prepare(
                "INSERT INTO urls(id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,
                   last_visit_iso,source_profile_id,created_by_run_id,source_url_id,page_id)
                 VALUES(?1,?2,'A page',0,'1970-01-01T00:00:00Z',0,'1970-01-01T00:00:00Z',?3,1,?1,?1)",
            )?;
            for id in 1..=URLS {
                let address = format!("https://site{id}.test/");
                page.execute(vault_store::params![id, address.clone(), format!("site{id}.test")])?;
                url.execute(vault_store::params![id, address, 1 + id % 2])?;
            }
            let mut download = s.prepare(
                "INSERT INTO downloads(id,source_download_id,guid,target_path,start_time_ms,
                   source_profile_id,created_by_run_id)
                 VALUES(?1,CAST(?1 AS TEXT),'guid-'||?1,'/downloads/'||?1,?1*1000,1+?1%2,1)",
            )?;
            for id in 1..=300_i64 {
                download.execute([id])?;
            }
            Ok(())
        })
        .unwrap();
    for start in (1..=visits).step_by(10_000) {
        store
            .write(Db::Archive, Priority::Batch, move |s| {
                let mut visit = s.prepare(
                    "INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,
                       source_profile_id,created_by_run_id,from_visit)
                     VALUES(?1,?2,CAST(?1 AS TEXT),1700000000000+?1*1000,'2023-11-14T22:13:20Z',
                       1+?1%2,1,?3)",
                )?;
                for id in start..=(start + 9_999).min(visits) {
                    visit.execute(vault_store::params![
                        id,
                        1 + (id - 1) % URLS,
                        opener(id, visits)
                    ])?;
                }
                Ok(())
            })
            .unwrap();
    }
    super::lineage::seed_v1(&store, embeddings);
}

/// Every table of every durable database, a digest per table in row order, and the schema.
fn contents(store: &Store) -> Vec<(String, u64, u64)> {
    let mut all = Vec::new();
    for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
        let found = store
            .read(db, move |s| {
                // The version table is the Store's own and cannot be read; this is its answer.
                let mut found = vec![(format!("{db:?} version"), s.schema_version()? as u64, 0)];
                let schema = s.query(
                    "SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name",
                    (),
                    row_digest,
                )?;
                found.push((format!("{db:?} schema"), schema.len() as u64, digest(&schema)));
                let tables = s.query(
                    "SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name",
                    (),
                    |r| r.get::<String>(0).map_err(Into::into),
                )?;
                for table in tables.into_iter().filter(|table| table != "__pk_migrations") {
                    let rows = s.query(
                        &format!("SELECT * FROM \"{table}\" ORDER BY rowid"),
                        (),
                        row_digest,
                    )?;
                    found.push((format!("{db:?}.{table}"), rows.len() as u64, digest(&rows)));
                }
                Ok(found)
            })
            .unwrap();
        all.extend(found);
    }
    all
}

fn row_digest(row: &vault_store::Row) -> vault_store::Result<u64> {
    let mut hasher = DefaultHasher::new();
    for column in 0..row.column_count() {
        format!("{:?}", row.get_value(column)?).hash(&mut hasher);
    }
    Ok(hasher.finish())
}

fn digest(rows: &[u64]) -> u64 {
    let mut hasher = DefaultHasher::new();
    rows.hash(&mut hasher);
    hasher.finish()
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
