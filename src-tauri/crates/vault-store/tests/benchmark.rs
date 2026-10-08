//! Responsibilities: reproducible 1M-row import measurements using the public facade.
//! Not responsible for: certifying 4-core target performance from a development host.
//! Failure modes: metrics include post-import work, imports omit rows, disk use
//! exceeds 3 GB, or fixtures survive success; assertions and TempDir cover these.
mod support;
use std::{
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Priority, Store};
fn uptime() -> String {
    String::from_utf8(Command::new("uptime").output().unwrap().stdout).unwrap().trim().into()
}
fn bytes(root: &Path) -> u64 {
    std::fs::read_dir(root)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            if p.is_dir() { bytes(&p) } else { std::fs::metadata(p).unwrap().len() }
        })
        .sum()
}
fn stats(mut values: Vec<f64>) -> (usize, f64, f64) {
    values.sort_by(f64::total_cmp);
    (values.len(), values[values.len() / 2], *values.last().unwrap())
}
#[test]
#[ignore = "1M-row measured import; run --release --ignored --nocapture --test-threads=1"]
fn million_visit_import_latency() {
    let (dir, store) = support::open();
    let root = dir.path().to_owned();
    let store = Arc::new(store);
    store
        .write(Db::Archive, Priority::Interactive, |tx| {
            tx.execute("CREATE INDEX visit_time ON visits(visited_at DESC,id DESC)", ())?;
            Ok(())
        })
        .unwrap();
    let load_start = uptime();
    let stop = Arc::new(AtomicBool::new(false));
    let worker = store.clone();
    let done = stop.clone();
    let observed_root = root.clone();
    let reader = thread::spawn(move || {
        let mut values = Vec::new();
        let mut peak_data = 0;
        let mut peak_wal = 0;
        while !done.load(Ordering::Acquire) {
            let start = Instant::now();
            worker
                .read(Db::Archive, |c| {
                    c.query(
                        "SELECT id,url,title,visited_at FROM visits ORDER BY visited_at DESC,id DESC LIMIT 100",
                        (),
                        |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
                    )
                })
                .unwrap();
            values.push(start.elapsed().as_secs_f64() * 1000.0);
            let disk = bytes(&observed_root);
            assert!(disk < 3_000_000_000);
            peak_data = peak_data.max(disk);
            peak_wal = peak_wal.max(
                std::fs::metadata(observed_root.join("archive/history-vault.db-wal"))
                    .unwrap()
                    .len(),
            );
            thread::sleep(Duration::from_millis(1));
        }
        (values, peak_data, peak_wal)
    });
    let (started_tx, started_rx) = mpsc::channel();
    let worker = store.clone();
    let importer = thread::spawn(move || {
        let mut peak = 0;
        let mut wal_peak = 0;
        for batch in 0_i64..100 {
            let started = started_tx.clone();
            worker
                .write(Db::Archive, Priority::Batch, move |tx| {
                    started.send(batch).unwrap();
                    tx.execute(
                        "WITH RECURSIVE seq(x) AS (
                        VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<10000
                     ) INSERT INTO visits(id,url,title,visited_at)
                     SELECT ?1+x, 'https://site'||(x%20000)||'.example/page/'||(?1+x),
                            'Visit title '||(?1+x), 1700000000+?1+x FROM seq",
                        [batch * 10000],
                    )?;
                    Ok(())
                })
                .unwrap();
            let size = bytes(&root);
            assert!(size < 3_000_000_000);
            peak = peak.max(size);
            wal_peak = wal_peak
                .max(std::fs::metadata(root.join("archive/history-vault.db-wal")).unwrap().len());
        }
        (peak, wal_peak)
    });
    let mut waits = Vec::new();
    let mut responses = Vec::new();
    while let Ok(batch) = started_rx.recv() {
        let arrival = Instant::now();
        let wait = store
            .write(Db::Archive, Priority::Interactive, move |tx| {
                let wait = arrival.elapsed().as_secs_f64() * 1000.0;
                tx.execute("UPDATE visits SET starred=1 WHERE id=?1", [batch * 10000 + 1])?;
                Ok(wait)
            })
            .unwrap();
        waits.push(wait);
        responses.push(arrival.elapsed().as_secs_f64() * 1000.0);
    }
    let (peak, wal_peak) = importer.join().unwrap();
    stop.store(true, Ordering::Release);
    let (reads, observed_data, observed_wal) = reader.join().unwrap();
    let peak = peak.max(observed_data);
    let wal_peak = wal_peak.max(observed_wal);
    assert_eq!(support::count(&store), 1_000_000);
    let starred = store
        .read(Db::Archive, |c| {
            c.query_one("SELECT count(*) FROM visits WHERE starred=1", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    assert_eq!(starred, 100);
    let load_end = uptime();
    println!(
        "host={}-{} cpus={} rows=1000000 batch_rows=10000",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().unwrap()
    );
    println!("uptime_start={load_start}\nuptime_end={load_end}");
    println!("interactive_wait_count_p50_max_ms={:?}", stats(waits));
    println!("interactive_response_count_p50_max_ms={:?}", stats(responses));
    println!("reader_count_p50_max_ms={:?}", stats(reads));
    println!(
        "peak_data_bytes={peak} peak_wal_bytes={wal_peak} metrics={:?}",
        store.metrics(Db::Archive)
    );
    let path = dir.path().to_owned();
    drop(store);
    dir.close().unwrap();
    assert!(!path.exists());
    println!("disposable_data_deleted=true");
}

/// Encrypted page work under the configured cipher: many small commits (the shape of an
/// Insights rebuild or a backup's outbox), a few large ones, then a cold scan after reopening.
#[test]
#[ignore = "encrypted write/read throughput; run --release --ignored --nocapture --test-threads=1"]
fn encrypted_page_throughput() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = support::config(dir.path());
    config.keys[Db::Archive as usize] = Some([7; 32]);
    let store = Store::open(config).unwrap();
    let insert = |tx: &vault_store::Session, first: i64, rows: i64| {
        tx.execute(
            "WITH RECURSIVE seq(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM seq WHERE x<?2-1)
             INSERT INTO visits(id,url,title,visited_at)
             SELECT ?1+x, 'https://site'||((?1+x)%20000)||'.example/page/'||(?1+x),
                    'Visit title '||(?1+x), 1700000000+?1+x FROM seq",
            (first, rows),
        )
    };
    let small = Instant::now();
    for commit in 0_i64..4_000 {
        store.write(Db::Archive, Priority::Batch, move |tx| insert(tx, commit * 100, 100)).unwrap();
    }
    let small = small.elapsed();
    let large = Instant::now();
    for commit in 0_i64..40 {
        let first = 400_000 + commit * 10_000;
        store.write(Db::Archive, Priority::Batch, move |tx| insert(tx, first, 10_000)).unwrap();
    }
    let large = large.elapsed();
    let root = dir.path().to_owned();
    store.close().unwrap();
    let mut config = support::config(&root);
    config.keys[Db::Archive as usize] = Some([7; 32]);
    let store = Store::open(config).unwrap();
    let scan = Instant::now();
    let total = store
        .read(Db::Archive, |c| {
            c.query_one("SELECT count(*), sum(length(url)+length(title)) FROM visits", (), |r| {
                Ok((r.get::<i64>(0)?, r.get::<i64>(1)?))
            })
        })
        .unwrap();
    let scan = scan.elapsed();
    assert_eq!(total.0, 800_000);
    println!(
        "cipher_bench host={}-{} small_commits_4000x100_s={:.2} large_commits_40x10000_s={:.2} cold_scan_800k_s={:.2} bytes={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        small.as_secs_f64(),
        large.as_secs_f64(),
        scan.as_secs_f64(),
        bytes(&root)
    );
}
