//! Responsibilities: measure native backup on a generated million-visit Chrome history: a first
//! full read, an hourly backup with nothing new, a weekly full read, and the first backup of a
//! version 1 archive after the update.
//! Not responsible for: pass/fail thresholds; the numbers go to the validation report.
//!
//! Synthetic data only. Every browser, keychain and scheduler path points into
//! `PATHKEEP_SCALE_ROOT`, so no real browser profile is discovered or read. Run each phase in its
//! own process under `/usr/bin/time -l`, in a release build:
//! `PATHKEEP_SCALE_ROOT=… PATHKEEP_SCALE_PHASE=source|backup|age [PATHKEEP_SCALE_ROWS=1000000]
//! cargo test --release -p vault-core --lib -- --ignored measure_native_backup --nocapture`.
use crate::{AppConfig, ArchiveMode, Vault, project_paths_with_root, run_backup, save_config};
use rusqlite::{Connection, params};
use std::{path::Path, time::Instant};

const KEY: [u8; 32] = [7; 32];
const CHROME_EPOCH_MICROS: i64 = 11_644_473_600_000_000;
const DAY_MICROS: i64 = 86_400_000_000;

#[test]
#[ignore = "scaled measurement; run its phases one by one in a release build"]
fn measure_native_backup() {
    let root = std::path::PathBuf::from(std::env::var("PATHKEEP_SCALE_ROOT").expect("root"));
    let phase = std::env::var("PATHKEEP_SCALE_PHASE").expect("phase");
    for (name, dir) in [
        ("CHB_CHROME_USER_DATA_DIR", "chrome-user-data"),
        ("CHB_FIREFOX_PROFILES_DIR", "firefox"),
        ("CHB_SAFARI_ROOT", "safari"),
        ("CHB_TEST_KEYRING_DIR", "keyring"),
        ("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", "scheduler"),
    ] {
        let path = root.join(dir);
        std::fs::create_dir_all(&path).unwrap();
        unsafe { std::env::set_var(name, &path) };
    }
    match phase.as_str() {
        "source" => {
            let rows = std::env::var("PATHKEEP_SCALE_ROWS")
                .map(|rows| rows.parse().unwrap())
                .unwrap_or(1_000_000_i64);
            let started = Instant::now();
            write_chrome(&root.join("chrome-user-data"), rows);
            eprintln!(
                "SCALE phase=source rows={rows} seconds={:.3}",
                started.elapsed().as_secs_f64()
            );
        }
        "backup" => backup(&root),
        "age" => age(&root),
        other => panic!("unknown phase {other}"),
    }
}

fn config() -> AppConfig {
    AppConfig {
        initialized: true,
        git_enabled: false,
        capture_favicons: false,
        archive_mode: ArchiveMode::Encrypted,
        selected_profile_ids: vec!["chrome:Default".into()],
        ..AppConfig::default()
    }
}

fn backup(root: &Path) {
    let paths = project_paths_with_root(&root.join("project"));
    crate::config::ensure_paths(&paths).unwrap();
    save_config(&paths, &config()).unwrap();
    let started = Instant::now();
    let vault = Vault::open(paths, config(), Some(KEY)).unwrap();
    let opened = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let report = run_backup(&vault, false).unwrap();
    let backed_up = started.elapsed().as_secs_f64();
    let run = report.run.expect("a run");
    // How many visits have their browser time and their opener by archive id: the update's
    // first backup gives every visit still in the browser its time, and the migration gives
    // version 1 visits their openers.
    let (visits, keyed, with_opener) = vault
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query_one(
                "SELECT COUNT(*),COUNT(source_visit_time),COUNT(parent_visit_id) FROM visits",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?)),
            )
        })
        .unwrap();
    let notes = report.warning_details.iter().map(|w| w.code.as_str()).collect::<Vec<_>>();
    vault.close().unwrap();
    eprintln!(
        "SCALE phase=backup open_seconds={opened:.3} backup_seconds={backed_up:.3} status={} new_visits={} new_urls={} archive_visits={visits} with_browser_time={keyed} with_opener={with_opener} notes={notes:?}",
        run.status, run.new_visits, run.new_urls
    );
}

/// The next backup reads every row, as it does once a week.
fn age(root: &Path) {
    let paths = project_paths_with_root(&root.join("project"));
    let vault = Vault::open(paths, config(), Some(KEY)).unwrap();
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(8)).to_rfc3339();
    vault
        .store()
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, move |s| {
            s.execute("UPDATE profile_watermarks SET last_full_scan_at=?1", [week_ago])?;
            Ok(())
        })
        .unwrap();
    vault.close().unwrap();
    eprintln!("SCALE phase=age");
}

/// A Chrome profile with `rows` visits over the last 90 days, one URL per five visits, every
/// second visit opened by the one before, a thousand downloads and ten thousand searches.
fn write_chrome(chrome: &Path, rows: i64) {
    let profile = chrome.join("Default");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(chrome.join("Last Version"), "146.0.0.0").unwrap();
    std::fs::write(
        chrome.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default"}}}}"#,
    )
    .unwrap();
    let path = profile.join("History");
    let _ = std::fs::remove_file(&path);
    let mut db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT,url TEXT NOT NULL,title TEXT,
           visit_count INTEGER NOT NULL,typed_count INTEGER NOT NULL,last_visit_time INTEGER NOT NULL,
           hidden INTEGER NOT NULL);
         CREATE TABLE visits(id INTEGER PRIMARY KEY AUTOINCREMENT,url INTEGER NOT NULL,
           visit_time INTEGER NOT NULL,from_visit INTEGER,transition INTEGER,visit_duration INTEGER,
           is_known_to_sync INTEGER,visited_link_id INTEGER,external_referrer_url TEXT,app_id TEXT);
         CREATE INDEX visits_time_index ON visits(visit_time);
         CREATE TABLE downloads(id INTEGER PRIMARY KEY,guid TEXT,current_path TEXT,target_path TEXT,
           start_time INTEGER,received_bytes INTEGER,total_bytes INTEGER,state INTEGER,
           mime_type TEXT,original_mime_type TEXT);
         CREATE TABLE keyword_search_terms(keyword_id INTEGER,url_id INTEGER,term TEXT,
           normalized_term TEXT);",
    )
    .unwrap();
    let urls = (rows / 5).max(1);
    let now = (chrono::Utc::now().timestamp_millis()) * 1_000 + CHROME_EPOCH_MICROS;
    let start = now - 90 * DAY_MICROS;
    let step = (90 * DAY_MICROS) / rows.max(1);
    let time = |i: i64| start + i * step + (i * 7919) % 997;
    let tx = db.transaction().unwrap();
    {
        let mut url = tx.prepare("INSERT INTO urls VALUES(?1,?2,?3,5,0,?4,0)").unwrap();
        for u in 1..=urls {
            // The URL's last visit is the last visit i with (i - 1) % urls == u - 1.
            let last = rows - ((rows - u).rem_euclid(urls));
            url.execute(params![
                u,
                format!("https://site{}.example/page/{u}", u % 5_000),
                format!("Page {u}"),
                time(last)
            ])
            .unwrap();
        }
        let mut visit = tx
            .prepare("INSERT INTO visits VALUES(?1,?2,?3,?4,805306368,20000000,0,0,NULL,NULL)")
            .unwrap();
        for i in 1..=rows {
            let opener = if i % 2 == 0 { i - 1 } else { 0 };
            visit.execute(params![i, (i - 1) % urls + 1, time(i), opener]).unwrap();
        }
        let mut download =
            tx.prepare("INSERT INTO downloads VALUES(?1,?2,'/tmp/a.part','/tmp/a',?3,10,10,1,'text/plain','text/plain')").unwrap();
        for d in 1..=1_000_i64 {
            download.execute(params![d, format!("guid-{d}"), time(d * 997 % rows + 1)]).unwrap();
        }
        let mut term = tx.prepare("INSERT INTO keyword_search_terms VALUES(2,?1,?2,?2)").unwrap();
        for k in 1..=10_000_i64 {
            term.execute(params![k * 31 % urls + 1, format!("term {k}")]).unwrap();
        }
    }
    tx.commit().unwrap();
}
