//! Responsibilities: seed bounded real archive/search/vector files for races and timing probes.
//! Not responsible for: simulating inference or replacing production grouping and persistence.
//! Failure modes: measurements omit trail hydration, or tests bypass the actual mapped sidecars.
//! Tests: search_group_snapshots_tests and the opt-in search_groups_bench probe.
use super::{
    AiProviderRuntime, EmbeddingFingerprint, ReverseVisitMap, VectorStore, VisitContentMap,
    search_groups_cluster::normalize, search_vectors,
};
use crate::{AiProviderConfig, AiProviderPurpose, Vault};
use anyhow::Result;
use chrono::{NaiveDate, TimeZone};
use vault_store::{Db, Priority, params_owned as params};

pub(super) fn vector(id: usize, dim: usize) -> Vec<f32> {
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
pub(super) fn seed(
    vault: &Vault,
    count: usize,
    month_count: usize,
    dim: usize,
) -> Result<AiProviderRuntime> {
    let provider = AiProviderRuntime {
        config: AiProviderConfig {
            id: "group-probe".into(),
            default_model: "model".into(),
            enabled: true,
            purpose: AiProviderPurpose::Embedding,
            base_url: Some("http://127.0.0.1:1/v1".into()),
            ..Default::default()
        },
        api_key: None,
    };
    let fp = EmbeddingFingerprint::from_descriptor(
        &super::embedding_external::external_descriptor(&provider, Some(dim)),
    )
    .unwrap();
    let page_store = VectorStore::create_stamped(vault.paths(), &fp)?;
    let search_store =
        VectorStore::for_provider(vault.paths(), &search_vectors::identity(&provider), "model")
            .create_stamped_at(vault.paths(), &fp, "normalized-query-v1".into())?;
    let pages = (0..count).map(|i| ((i + 1) as u64, vector(i + count, dim))).collect::<Vec<_>>();
    page_store.append_vectors(&pages)?;
    search_store.append_vectors(
        &(0..count)
            .map(|i| (search_vectors::key(&format!("query {i}")), vector(i, dim)))
            .collect::<Vec<_>>(),
    )?;
    let mut records = Vec::new();
    for month in 1..=month_count {
        let first = NaiveDate::from_ymd_opt(2025, month as u32, 1).unwrap();
        for i in 0..count {
            let day = first + chrono::Duration::days((i % 28) as i64);
            let time = chrono_tz::UTC
                .from_utc_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
                .timestamp_millis()
                + (i / 28) as i64 * 60_000;
            let id = ((month - 1) * count + i) as i64 * 2 + 1;
            records.push((id, time, i as i64, format!("trail:{month}:{i}")));
        }
    }
    vault.write(Db::Archive,Priority::Batch,move |s| {
        s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES(1,'probe','chrome','Probe','/fixture','now')",())?;
        for i in 0..=count {
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'example.com')",params![i as i64+1,format!("https://example.com/{i}")])?;
            s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(?1,?1,?2,'Probe',0,'',0,'',1,0,?1)",params![i as i64+1,format!("https://example.com/{i}")])?;
        }
        Ok(())
    })?;
    let rows = records.clone();
    vault.write(Db::Archive,Priority::Batch,move |s| {
        let mut insert = s.prepare("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(?1,?2,?1,?3,'',1,0)")?;
        for (id,time,page,_) in rows {
            insert.execute(params![id,1,time])?;
            insert.execute(params![id+1,page+2,time+1000])?;
        }
        Ok(())
    })?;
    let rows = records.clone();
    vault.write(Db::Intelligence,Priority::Batch,move |s| {
        s.execute("INSERT INTO intelligence_generations(profile_id,navigation,time_zone) VALUES('probe',1,'UTC')",())?;
        let mut event = s.prepare("INSERT INTO search_events_rows(generation,visit_id,profile_id,search_engine,raw_query,normalized_query,trail_id,computed_at,visit_time_ms) VALUES(1,?1,'probe','google',?2,?2,?3,'now',?4)")?;
        let mut member = s.prepare("INSERT INTO search_trail_members_rows(generation,trail_id,profile_id,visit_id,ordinal,role) VALUES(1,?1,'probe',?2,1,'landing')")?;
        for (id,time,page,trail) in rows {
            event.execute(params![id,format!("query {page}"),trail,time])?;
            member.execute(params![trail,id+1])?;
        }
        Ok(())
    })?;
    let map = VisitContentMap::for_provider(vault.paths(), &provider.config.id, "model");
    map.ensure_created(vault.paths())?;
    map.append(
        &records.into_iter().map(|(id, _, i, _)| (id + 1, (i + 1) as u64)).collect::<Vec<_>>(),
    )?;
    project(vault, &provider)?;
    Ok(provider)
}
pub(super) fn project(vault: &Vault, provider: &AiProviderRuntime) -> Result<()> {
    let paths = vault.paths();
    let id = &provider.config.id;
    let model = &provider.config.default_model;
    super::build_planes_from_store(paths, id, model)?;
    super::build_planes_from_store(paths, &search_vectors::identity(provider), model)?;
    let hash = VectorStore::for_provider(paths, id, model).read_header()?.unwrap().fingerprint_hash;
    let map = VisitContentMap::for_provider(paths, id, model);
    ReverseVisitMap::for_provider(paths, id, model).build_from_visit_map(paths, &map, &hash)?;
    Ok(())
}
