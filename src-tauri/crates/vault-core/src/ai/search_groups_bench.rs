//! Responsibilities: measure production linkage and the complete search-vector drill-in read.
//! Not responsible for: inference speed or judging semantic quality.
//! Failure modes: a probe silently skips work, measures debug code, or omits database hydration.
//! Tests: opt-in release probe; outputs repeatable JSONL timings.
use super::{
    EmbeddingDtype, EmbeddingFingerprint, EmbeddingPooling, FlatVectorIndex, VectorStore,
    search_groups_cluster::{cluster, normalize},
    similar_searches::{SimilarSearches, read},
};
use anyhow::{Context, Result, ensure};
use std::{fs::OpenOptions, io::Write, time::Instant};
use vault_store::{Db, Priority, params_owned as params};

fn vector(id: usize, dim: usize) -> Vec<f32> {
    let mut seed = id as u64 + 1;
    let mut row = (0..dim)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1_u32 << 23) as f32 - 1.
        })
        .collect::<Vec<_>>();
    normalize(&mut row);
    row
}
#[test]
#[ignore = "opt-in search-group measurement; --release --ignored --nocapture"]
fn search_groups_measurement() -> Result<()> {
    if std::env::var("PATHKEEP_SEARCH_GROUPS_BENCH").as_deref() != Ok("1") {
        eprintln!("SKIP: set PATHKEEP_SEARCH_GROUPS_BENCH=1");
        return Ok(());
    }
    ensure!(!cfg!(debug_assertions), "measure with --release");
    let out = std::env::var_os("PATHKEEP_SEARCH_GROUPS_OUT")
        .context("set PATHKEEP_SEARCH_GROUPS_OUT to a JSONL artifact")?;
    let mut artifact = OpenOptions::new().create(true).append(true).open(out)?;
    for dim in [256, 1024] {
        let rows = (0..4096).map(|id| vector(id, dim)).collect::<Vec<_>>();
        for run in 0..3 {
            let start = Instant::now();
            let groups = cluster(&rows, &|| Ok(()))?;
            let record = serde_json::json!({"operation":"month linkage","searches":4096,"dim":dim,"run":run,"groups":groups.len(),"ms":start.elapsed().as_secs_f64()*1000.});
            writeln!(artifact, "{record}")?;
            eprintln!("{record}");
        }
        // Each dimension's temp Vault is dropped before the next; no second build target.
        let vault = crate::Vault::open_temp()?;
        let fp = EmbeddingFingerprint::new(
            "search-probe",
            "model",
            dim,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        );
        let store = VectorStore::create_stamped(vault.paths(), &fp)?;
        for start in (0..1_000_000).step_by(4096) {
            let rows = (start..(start + 4096).min(1_000_000))
                .map(|id| (super::search_vectors::key(&format!("search {id}")), vector(id, dim)))
                .collect::<Vec<_>>();
            store.append_vectors(&rows)?;
        }
        // Current-generation rows can outlive an undo until Insights publishes its next build.
        // Seed actual visible archive visits so the production visibility join is measured too.
        vault.write(Db::Archive, Priority::Batch, |session| {
            session.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES(1,'probe','chrome','Probe','/fixture','now')", ())?;
            session.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://www.google.com/search','https://www.google.com/search','google.com')", ())?;
            session.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(1,1,'https://www.google.com/search','Search',0,'1970-01-01T00:00:00Z',999999,'1970-01-01T00:16:39.999Z',1,0,1)", ())?;
            let mut insert = session.prepare("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(?1,1,?1,?2,?3,1,0)")?;
            for id in 0..1_000_000 {
                let time = id as i64;
                let iso = chrono::DateTime::from_timestamp_millis(time).unwrap().to_rfc3339();
                insert.execute(params![time + 1, time, iso])?;
            }
            Ok(())
        })?;
        vault.write(Db::Intelligence,Priority::Batch,|s|{
            s.execute("INSERT INTO intelligence_generations(profile_id,navigation) VALUES('probe',1)",())?;
            let mut insert=s.prepare("INSERT INTO search_events_rows(generation,visit_id,profile_id,search_engine,raw_query,normalized_query,computed_at,visit_time_ms) VALUES(1,?1,'probe','google',?2,?2,'now',?3)")?;
            for id in 0..1_000_000 {insert.execute(params![id as i64 + 1,format!("search {id}"), id as i64])?;}
            Ok(())
        })?;
        super::build_planes_from_store(vault.paths(), "search-probe", "model")?;
        let index = FlatVectorIndex::open(vault.paths(), "search-probe", "model")?;
        ensure!(index.len() == 1_000_000, "probe must read 1M distinct searches");
        for run in 0..3 {
            let start = Instant::now();
            let result = read(&vault, &index, "search 1234", &[], false)?;
            let SimilarSearches::Ready { items, .. } = result else {
                anyhow::bail!("probe unavailable: {result:?}");
            };
            ensure!(items.len() == 8, "probe must hydrate eight rows");
            let record = serde_json::json!({"operation":"drill-in read","searches":1_000_000,"dim":dim,"run":run,"rows":items.len(),"ms":start.elapsed().as_secs_f64()*1000.});
            writeln!(artifact, "{record}")?;
            eprintln!("{record}");
        }
    }
    Ok(())
}
