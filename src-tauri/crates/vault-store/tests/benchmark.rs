//! Responsibilities: reproducible encrypted write/read throughput measurements using the
//! public facade.
//! Not responsible for: certifying 4-core target performance from a development host.
//! Failure modes: writes omit rows or the cold scan reads a different count; assertions
//! cover these, and TempDir removes the data.
mod support;
use std::{path::Path, time::Instant};
use vault_store::{Db, Priority, Store};
fn bytes(root: &Path) -> u64 {
    std::fs::read_dir(root)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            if p.is_dir() { bytes(&p) } else { std::fs::metadata(p).unwrap().len() }
        })
        .sum()
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
