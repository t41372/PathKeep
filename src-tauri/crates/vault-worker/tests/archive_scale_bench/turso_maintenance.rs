//! Responsibilities: measure, at the product scale, the whole-archive operations a user waits
//! through, run by the worker's commands as the desktop runs them: the first open of an archive
//! an earlier release made (a newer build brings it up to its layout there), an encryption change,
//! a restore from the safety copy that change made, the export of a move file and its import, and
//! a restore that rewrites a copy under the archive's own encryption. For each: the wall time;
//! what the progress screen had to show, and the longest it had nothing new to show; the bytes
//! the process read and wrote; its peak memory; and the archive's size.
//! Not responsible for: limiting memory (the runner puts each step under the target machine's
//! 8 GB, page cache included: a larger cache would hold the archive and hide the reads), comparing
//! builds (the runner runs the steps with each build on a copy of one seeded folder), or another
//! machine's figures.
//! Failure modes: a smaller archive reported as the product scale (each step prints the visits it
//! finds); a still screen missed between samples (sampled every 100 ms, the screen asks every
//! 500 ms); a step run on what an earlier one left unfinished (each checks what it starts from);
//! the data left behind (all of it is under the folder the runner gives, which it removes).
//!
//! One step per run, each in its own process: `PATHKEEP_MAINTENANCE_BENCH_ROOT` names an empty
//! folder for the first, the same one after, and `PATHKEEP_MAINTENANCE_BENCH_STEP` is `seed`,
//! `open`, `mode-switch`, `restore`, `export`, `import` or `restore-converting`, in that order.
//! `seed` makes the reads benchmark's 14.4M visits (`PATHKEEP_BENCH_SHAPE=habits` for real
//! browsing's shape), the search index and Insights, in an encrypted archive. `export` deletes the
//! safety copies first: a move file carries them, and builds that keep different copies would
//! otherwise export and import different amounts. `restore-converting` needs a build that keeps
//! what an import replaces as a safety copy.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use vault_core::{AppConfig, ArchiveMode, Vault};
use vault_store::Db;
use vault_worker::VaultSession;

const PASSWORD: &str = "synthetic maintenance benchmark password";
const EXPORT_PASSWORD: &str = "synthetic maintenance benchmark export password";

#[test]
#[ignore = "14.4M maintenance benchmark: one step per run, in PATHKEEP_MAINTENANCE_BENCH_ROOT"]
fn maintenance_at_scale() {
    super::COUNT_HEAP.store(false, Ordering::Relaxed);
    let root = PathBuf::from(
        std::env::var_os("PATHKEEP_MAINTENANCE_BENCH_ROOT")
            .expect("PATHKEEP_MAINTENANCE_BENCH_ROOT names the benchmark's folder"),
    );
    let step = std::env::var("PATHKEEP_MAINTENANCE_BENCH_STEP")
        .expect("PATHKEEP_MAINTENANCE_BENCH_STEP names the step");
    isolate(&root);
    match step.as_str() {
        "seed" => seed(),
        "open" => first_open(),
        "mode-switch" => mode_switch(),
        "restore" => restore(),
        "export" => export(&root),
        "import" => import(&root),
        "restore-converting" => restore_converting(),
        other => panic!("unknown step {other}"),
    }
    println!(
        "BENCH step={step} peak_rss_mib={}",
        super::turso_reads::peak_rss_bytes().map_or(-1, |bytes| (bytes >> 20) as i64)
    );
    print_sizes(&step, &root.join("data"));
}

/// Points every path PathKeep uses at the benchmark's folder: the archive, the browsers it would
/// back up, the keychain and the schedule. Nothing reaches the network.
fn isolate(root: &Path) {
    let overrides = [
        ("CHB_PROJECT_ROOT", root.join("data")),
        ("CHB_CHROME_USER_DATA_DIR", root.join("chrome")),
        ("CHB_FIREFOX_PROFILES_DIR", root.join("firefox")),
        ("CHB_SAFARI_ROOT", root.join("safari")),
        ("CHB_TEST_KEYRING_DIR", root.join("keyring")),
        ("PATHKEEP_PLATFORM_TEST_KEYRING_DIR", root.join("keyring")),
        ("PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE", root.join("synthetic-key-service")),
        ("PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL", root.join("synthetic-schedule")),
        ("PATHKEEP_PLATFORM_TEST_LAUNCH_AGENTS_DIR", root.join("launch-agents")),
        ("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", root.join("platform")),
        ("PATHKEEP_TEST_SAVE_DIR", root.join("saved")),
        ("PATHKEEP_TEST_BLOCK_NETWORK", PathBuf::from("1")),
    ];
    for (name, value) in overrides {
        // One test in this process, before any thread starts.
        unsafe { std::env::set_var(name, value) };
    }
}

fn seed() {
    let session = VaultSession::default();
    let mut config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Encrypted,
        git_enabled: false,
        checkpoint_days: 1,
        selected_profile_ids: vec!["chrome:Default".into()],
        ..Default::default()
    };
    // Nothing runs in the background while a step is measured: Insights are built here, once.
    config.ai.job_queue_paused = true;
    config.ai.content_fetch_enabled = false;
    config.ai.auto_index_after_backup = false;
    vault_worker::initialize_archive_database_with_progress(
        &session,
        &config,
        Some(PASSWORD),
        |_| {},
    )
    .unwrap();
    let vault = session.vault().unwrap();
    let started = Instant::now();
    super::turso_reads::seed(&vault);
    println!("BENCH step=seed visits_seconds={:.1}", started.elapsed().as_secs_f64());
    let started = Instant::now();
    vault.search().rebuild().unwrap();
    println!("BENCH step=seed search_index_seconds={:.1}", started.elapsed().as_secs_f64());
    let started = Instant::now();
    vault_core::run_core_intelligence_with_progress(
        &vault,
        &vault_core::CoreIntelligenceRebuildRequest {
            full_rebuild: true,
            limit: Some(2048),
            ..Default::default()
        },
        |_| Ok(()),
    )
    .unwrap();
    println!("BENCH step=seed insights_seconds={:.1}", started.elapsed().as_secs_f64());
    print_visits("seed", &vault);
    session.close_vault().unwrap();
}

/// The first open by this build, which brings an archive an earlier release made up to its
/// layout, then a second for comparison.
fn first_open() {
    let session = VaultSession::default();
    let started = Instant::now();
    let vault = open(&session);
    println!("BENCH step=open first_seconds={:.2}", started.elapsed().as_secs_f64());
    print_visits("open", &vault);
    session.close_vault().unwrap();
    let started = Instant::now();
    open(&session);
    println!("BENCH step=open second_seconds={:.2}", started.elapsed().as_secs_f64());
    session.close_vault().unwrap();
}

fn mode_switch() {
    let session = VaultSession::default();
    let vault = open(&session);
    assert_eq!(vault.config().archive_mode, ArchiveMode::Encrypted, "seeded encrypted");
    print_visits("mode-switch", &vault);
    measure("mode-switch", || {
        vault_worker::rekey_archive_database(
            &session,
            &vault_worker::RekeyRequest {
                new_mode: ArchiveMode::Plaintext,
                new_key: None,
                current_key: Some(PASSWORD.into()),
                remember_in_keychain: None,
            },
        )
        .unwrap();
    });
    assert_eq!(open(&session).config().archive_mode, ArchiveMode::Plaintext);
    let _ = session.close_vault();
}

/// From the copy the encryption change made: the archive after the change, so in the same mode.
fn restore() {
    let session = VaultSession::default();
    let vault = open(&session);
    assert_eq!(vault.config().archive_mode, ArchiveMode::Plaintext, "after the mode switch");
    let copy = safety_copy("restore", "rekey-");
    measure("restore", || {
        vault_worker::run_full_archive_restore(
            &session,
            None,
            &vault_core::SnapshotRestoreRequest { snapshot_path: copy.path.clone() },
        )
        .unwrap();
    });
    print_visits("restore", &open(&session));
    let _ = session.close_vault();
}

fn export(root: &Path) {
    let session = VaultSession::default();
    let vault = open(&session);
    let copies = root.join("data/raw-snapshots/maintenance");
    for copy in std::fs::read_dir(&copies).into_iter().flatten() {
        std::fs::remove_dir_all(copy.unwrap().path()).unwrap();
    }
    let target = move_file(root);
    measure("export", || {
        vault_worker::export_app_data(&vault, target.clone(), EXPORT_PASSWORD).unwrap();
    });
    println!("BENCH step=export file_bytes={}", std::fs::metadata(&target).unwrap().len());
    let _ = session.close_vault();
}

fn import(root: &Path) {
    let session = VaultSession::default();
    let vault = open(&session);
    let paths = vault_core::project_paths().unwrap();
    measure("import", || {
        let (_, installed) = vault_worker::apply_import(
            &paths,
            Some(&vault),
            session.owner(),
            move_file(root),
            vault_core::ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some(EXPORT_PASSWORD.into()),
            },
        )
        .unwrap();
        print_visits("import", &installed.vault);
        installed.vault.close().unwrap();
    });
}

/// A copy in another mode than the archive's, written under the archive's encryption as it is
/// restored: after the import the archive is encrypted, and the archive the import replaced,
/// kept as a safety copy, is not.
fn restore_converting() {
    let session = VaultSession::default();
    let vault = open(&session);
    assert_eq!(vault.config().archive_mode, ArchiveMode::Encrypted, "after the import");
    let copy = safety_copy("restore-converting", "before-import-");
    measure("restore-converting", || {
        vault_worker::run_full_archive_restore(
            &session,
            None,
            &vault_core::SnapshotRestoreRequest { snapshot_path: copy.path.clone() },
        )
        .unwrap();
    });
    print_visits("restore-converting", &open(&session));
    let _ = session.close_vault();
}

/// The safety copy whose folder name starts with `kind`, after listing every copy there is.
fn safety_copy(step: &str, kind: &str) -> vault_core::RecoverySnapshot {
    let copies = vault_worker::list_recovery_snapshots().unwrap();
    let name = |copy: &vault_core::RecoverySnapshot| {
        Path::new(&copy.path).file_name().unwrap().to_string_lossy().into_owned()
    };
    for copy in &copies {
        println!("BENCH step={step} copy {}={}", name(copy), copy.size_bytes);
    }
    copies.into_iter().find(|copy| name(copy).starts_with(kind)).expect("the copy to restore")
}

fn move_file(root: &Path) -> PathBuf {
    root.join("move").join("benchmark.pathkeep")
}

/// Opens the archive as the desktop does: with its password when it has one. After the import it
/// has the move file's.
fn open(session: &VaultSession) -> Vault {
    let paths = vault_core::project_paths().unwrap();
    let config = vault_core::load_config(&paths).unwrap();
    if config.archive_mode == ArchiveMode::Plaintext {
        return session.vault().unwrap();
    }
    session
        .open_password(paths.clone(), config.clone(), PASSWORD.into())
        .or_else(|_| session.open_password(paths, config, EXPORT_PASSWORD.into()))
        .unwrap()
}

fn print_visits(step: &str, vault: &Vault) {
    let visits = vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
                Ok(r.get::<i64>(0)?)
            })
        })
        .unwrap();
    println!("BENCH step={step} visits={visits}");
}

/// The bytes under each folder of the data folder, and in all.
fn print_sizes(step: &str, data: &Path) {
    let Ok(entries) = std::fs::read_dir(data) else { return };
    let mut total = 0;
    for entry in entries {
        let entry = entry.unwrap();
        let bytes = if entry.file_type().unwrap().is_dir() {
            super::turso_reads::directory_bytes(&entry.path())
        } else {
            entry.metadata().unwrap().len()
        };
        total += bytes;
        println!("BENCH step={step} size {}={bytes}", entry.file_name().to_string_lossy());
    }
    println!("BENCH step={step} size total={total}");
}

/// Runs one operation and reports its time, what the progress screen showed meanwhile, and the
/// bytes the process read and wrote while it ran.
fn measure(step: &str, work: impl FnOnce()) {
    let before = Io::now();
    let started = Instant::now();
    let watch = Watch::start(started);
    work();
    let elapsed = started.elapsed();
    let after = Io::now();
    println!("BENCH step={step} seconds={:.1}", elapsed.as_secs_f64());
    if let (Some(before), Some(after)) = (before, after) {
        println!(
            "BENCH step={step} read_bytes={} written_bytes={} disk_read_bytes={} disk_written_bytes={}",
            after.read - before.read,
            after.written - before.written,
            after.disk_read - before.disk_read,
            after.disk_written - before.disk_written,
        );
    }
    watch.finish(step, elapsed);
}

/// What the process has read and written (Linux): through system calls, cached or not, and from
/// and to the disk itself.
#[derive(Clone, Copy)]
struct Io {
    read: u64,
    written: u64,
    disk_read: u64,
    disk_written: u64,
}

impl Io {
    fn now() -> Option<Self> {
        let text = std::fs::read_to_string("/proc/self/io").ok()?;
        let field = |name: &str| {
            text.lines().find_map(|line| line.strip_prefix(name)?.strip_prefix(": ")?.parse().ok())
        };
        Some(Self {
            read: field("rchar")?,
            written: field("wchar")?,
            disk_read: field("read_bytes")?,
            disk_written: field("write_bytes")?,
        })
    }
}

/// What the progress screen would show: the step, the part of the archive it is on, and its
/// numbers.
#[derive(Clone, PartialEq)]
struct Shown {
    phase: String,
    part: String,
    processed: u64,
    total: u64,
}

/// Samples the progress the screen reads, more often than the screen reads it.
struct Watch {
    stop: Arc<AtomicBool>,
    samples: std::thread::JoinHandle<Vec<(Duration, Option<Shown>)>>,
}

impl Watch {
    fn start(started: Instant) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let samples = std::thread::spawn(move || {
            let mut samples: Vec<(Duration, Option<Shown>)> = Vec::new();
            while !stopped.load(Ordering::Relaxed) {
                let shown = vault_worker::maintenance_progress().ok().flatten().map(|p| Shown {
                    phase: p.phase,
                    part: [p.database, p.table].into_iter().flatten().collect::<Vec<_>>().join("/"),
                    processed: p.processed,
                    total: p.total,
                });
                if samples.last().is_none_or(|(_, last)| *last != shown) {
                    samples.push((started.elapsed(), shown));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            samples
        });
        Self { stop, samples }
    }

    /// One line for each stretch of one phase on one part: when, whether its numbers moved, and
    /// the longest nothing on the screen changed; then the longest for the whole operation.
    fn finish(self, step: &str, elapsed: Duration) {
        self.stop.store(true, Ordering::Relaxed);
        let samples = self.samples.join().unwrap();
        let ends = samples.iter().skip(1).map(|(at, _)| *at).chain([elapsed]);
        let mut stretches: Vec<(String, Duration, Duration, usize, bool, Duration)> = Vec::new();
        let mut still = (Duration::ZERO, Duration::ZERO, String::new());
        for ((at, shown), end) in samples.iter().zip(ends) {
            let name = shown
                .as_ref()
                .map_or("(none)".to_owned(), |shown| format!("{} {}", shown.phase, shown.part));
            let quiet = end.saturating_sub(*at);
            if quiet > still.0 {
                still = (quiet, *at, name.clone());
            }
            let counted = shown.as_ref().is_some_and(|shown| shown.total > 0);
            match stretches.last_mut() {
                Some(last) if last.0 == name => {
                    last.2 = end;
                    last.3 += 1;
                    last.4 |= counted;
                    last.5 = last.5.max(quiet);
                }
                _ => stretches.push((name, *at, end, 1, counted, quiet)),
            }
        }
        for (name, from, to, changes, counted, quiet) in stretches {
            println!(
                "BENCH step={step} shown \"{name}\" from_s={:.1} to_s={:.1} changes={changes} \
                 with_total={counted} longest_still_s={:.1}",
                from.as_secs_f64(),
                to.as_secs_f64(),
                quiet.as_secs_f64(),
            );
        }
        println!(
            "BENCH step={step} longest_still_s={:.1} at_s={:.1} showing \"{}\"",
            still.0.as_secs_f64(),
            still.1.as_secs_f64(),
            still.2
        );
    }
}
