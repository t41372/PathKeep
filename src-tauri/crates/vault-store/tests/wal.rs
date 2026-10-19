//! Responsibilities: measure WAL and checkpoint behavior during a real import.
//! Not responsible for: bounding WAL under arbitrary long-running caller closures.
//! Failure modes: short readers pin WAL across all 100 commits; idle never truncates;
//! auto-checkpoint is assumed configurable instead of observed.
mod support;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use vault_store::{Db, Priority};
#[test]
fn wal_is_bounded_across_100_batch_commits_with_active_readers() {
    let (dir, store) = support::open();
    let store = Arc::new(store);
    let stop = Arc::new(AtomicBool::new(false));
    let worker = store.clone();
    let done = stop.clone();
    let reader = thread::spawn(move || {
        let mut reads = 0;
        while !done.load(Ordering::Acquire) {
            support::count(&worker);
            reads += 1;
        }
        reads
    });
    let wal = dir.path().join("archive/history-vault.db-wal");
    let mut peak = 0;
    for batch in 0..100 {
        store.write(Db::Archive,Priority::Batch,move |tx|{
            tx.execute("WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<100) INSERT INTO visits(id,url,title,visited_at) SELECT ?1+x,'https://example.test',hex(zeroblob(500)),x FROM seq",[batch*100])?;Ok(())
        }).unwrap();
        peak = peak.max(std::fs::metadata(&wal).unwrap().len());
    }
    stop.store(true, Ordering::Release);
    assert!(reader.join().unwrap() > 0);
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::fs::metadata(&wal).unwrap().len() != 0 {
        assert!(Instant::now() < deadline, "idle writer did not truncate");
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(support::count(&store), 10000);
    assert!(peak < 2 * 1024 * 1024, "WAL grew with total import: {peak}");
    let metrics = store.metrics(Db::Archive);
    assert_eq!(metrics.passive_checkpoints, 100);
    assert!(metrics.truncate_checkpoints > 0);
    println!("wal_peak_bytes={peak} metrics={metrics:?}");
}
