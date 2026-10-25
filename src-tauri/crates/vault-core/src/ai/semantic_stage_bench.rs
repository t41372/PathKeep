//! Responsibilities: measure the real semantic pipeline with ordinary archive/index files.
//! Not responsible for: embedding model speed, production changes, or substituting fake storage.
//!
//! Stage probes call the same private functions as History and cross-check the public result.

use super::*;
use crate::ai::traits::{EmbeddingDescriptor, EmbeddingProvider};
use crate::ai::{
    EmbeddingFingerprint, ReverseVisitMap, VectorStore, VisitContentMap, build_planes_from_store,
};
use anyhow::{Context, ensure};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use vault_store::{Db, Priority, params};

const DIM: usize = 256;
const QUERIES: usize = 20;

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    }
    fn vector(&mut self, dim: usize) -> Vec<f32> {
        let mut row = (0..dim)
            .map(|_| (self.next() >> 40) as f32 / (1_u32 << 23) as f32 - 1.)
            .collect::<Vec<_>>();
        let norm = row.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut row {
            *x /= norm;
        }
        row
    }
}

pub(in crate::ai) fn seed(
    vault: &Vault,
    visits: i64,
    provider: &AiProviderRuntime,
    dim: usize,
) -> Result<()> {
    let pages = (visits + 1) / 2;
    // Prepared statements, one transaction, one URL per page; each page has two visible visits.
    // Search documents/outbox are produced by the same refresh path used by archive writes.
    vault.store().write(Db::Archive, Priority::Batch, move |s| {
        s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES(1,'chrome:Default','chrome','Default','/fixture','now')", ())?;
        let mut page=s.prepare("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3)")?;
        let mut url=s.prepare("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(?1,?1,?2,?3,?4,?6,?5,?7,1,0,?1)")?;
        let mut visit=s.prepare("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(?1,?2,?1,?3,?4,1,0)")?;
        let span=730*86_400_000_i64;
        for id in 1..=pages {
            let domain=format!("site{}.example",id%2000);
            let address=format!("https://{domain}/page/{id}");
            let first=1_704_067_200_000+(id-1)*span/pages;
            let last=if id*2<=visits {first+86_400_000} else {first};
            let first_iso=chrono::DateTime::from_timestamp_millis(first).unwrap().to_rfc3339();
            let last_iso=chrono::DateTime::from_timestamp_millis(last).unwrap().to_rfc3339();
            page.execute(params![id,address.as_str(),domain])?;
            url.execute(params![id,address,format!("Page {id} topic{}",id%20),first,last,first_iso,last_iso])?;
            for (vid,time) in [(id*2-1,first),(id*2,last)] {
                if vid>visits { continue; }
                let iso=chrono::DateTime::from_timestamp_millis(time).unwrap().to_rfc3339();
                visit.execute(params![vid,id,time,iso])?;
            }
            crate::archive::search_documents::refresh(s,id)?;
        }
        for (name,value) in [("visits",visits),("urls",pages),("sourceProfiles",1),("sourceVisits:1",visits)] {
            s.execute("INSERT INTO archive_totals(name,value) VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET value=excluded.value",params![name,value])?;
        }
        Ok(())
    })?;
    vault.search().cycle()?;
    let paths = vault.paths();
    let engine = crate::ai::embedding_engine::select_embedding_provider(paths, provider)?;
    let fingerprint = EmbeddingFingerprint::from_descriptor(&EmbeddingDescriptor {
        provider_id: provider.config.id.clone(),
        model_id: provider.config.default_model.clone(),
        effective_dim: Some(dim),
        ..engine.descriptor()
    })
    .unwrap();
    let store = VectorStore::create_stamped(paths, &fingerprint)?;
    let map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    map.ensure_created(paths)?;
    let mut random = Random(0x5041_5448_4b45_4550);
    for start in (1..=pages).step_by(4096) {
        let end = (start + 4096).min(pages + 1);
        let vectors = (start..end).map(|id| (id as u64, random.vector(dim))).collect::<Vec<_>>();
        store.append_vectors(&vectors)?;
        let mappings = (start..end)
            .flat_map(|id| [(id * 2 - 1, id as u64), (id * 2, id as u64)])
            .filter(|(id, _)| *id <= visits)
            .collect::<Vec<_>>();
        map.append(&mappings)?;
    }
    build_planes_from_store(paths, &provider.config.id, &provider.config.default_model)?;
    ReverseVisitMap::for_provider(paths, &provider.config.id, &provider.config.default_model)
        .build_from_visit_map(paths, &map, &fingerprint.hash())?;
    let watermark = crate::ai::native_ledger::current_source_watermark(vault)?;
    let provider = provider.clone();
    vault.store().write(Db::Intelligence,Priority::Batch,move|s| {
        let mut row=s.prepare("INSERT INTO ai_embeddings(history_id,profile_id,url,title,domain,visited_at,content_hash,content_bytes,metadata_bytes,provider_id,model,indexed_at,lineage) VALUES(?1,'chrome:Default',?2,?3,?4,'2026-01-01T00:00:00Z',?5,64,0,?6,?7,'2026-10-16T00:00:00Z','legacy')")?;
        for id in 1..=visits {
            let page=(id+1)/2;
            let domain=format!("site{}.example",page%2000);
            row.execute(params![id,format!("https://{domain}/page/{page}"),format!("Page {page} topic{}",page%20),domain,format!("page-{page}"),provider.config.id.as_str(),provider.config.default_model.as_str()])?;
        }
        s.execute("INSERT INTO ai_embedding_totals(provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at,lineage) VALUES(?1,?2,?3,?4,0,'2026-10-16T00:00:00Z','legacy')",params![provider.config.id.as_str(),provider.config.default_model.as_str(),visits,visits*64])?;
        s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,source_watermark,last_run_id,build_started_at,build_finished_at,last_indexed_at) VALUES(?1,?2,'ai_embeddings','stage-bench','ready',?3,1,'2026-10-16T00:00:00Z','2026-10-16T00:00:00Z','2026-10-16T00:00:00Z')",params![provider.config.id.as_str(),provider.config.default_model.as_str(),watermark])?;
        Ok(())
    })?;
    super::super::generations::migrate_flat_indexes(vault)?;
    Ok(())
}

#[derive(Serialize)]
struct Probe {
    query: String,
    lexical_ms: f64,
    semantic_ms: f64,
    scope_ms: f64,
    rank_fuse_ms: f64,
    first_page_ms: f64,
    total_ms: f64,
    public_ms: f64,
    pages: usize,
    public_items_match: bool,
}
fn release_cursor(vault: &Vault, cursor: Option<&str>) -> Result<()> {
    if let Some(cursor) = cursor {
        let (session, _) = cursor.split_once(':').context("invalid search cursor")?;
        vault.search_sessions().cancel(session)?;
    }
    Ok(())
}

fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.
}

fn measured_page(
    vault: &Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
    vector: &[f32],
) -> Result<(Probe, Vec<i64>)> {
    let total = Instant::now();
    let scope = ranked_query(config, Some(provider), request)?;
    let mut session = SearchSession::start(vault, scope.clone())?;
    let ranks = lexical_ranks(&mut session)?;
    let cancel = session.cancel_token()?;
    let lexical_ms = elapsed(total);
    let start = Instant::now();
    let mut notes = Vec::new();
    let index =
        semantic_index(vault, paths, provider, &mut notes)?.context("empty seeded index")?;
    let semantic = semantic_scores(paths, provider, &index, vector, &cancel, None, &mut notes)?
        .context("seeded index drifted")?;
    ensure!(notes.is_empty(), "seeded semantic stage degraded: {notes:?}");
    let semantic_ms = elapsed(start);
    let start = Instant::now();
    let ranks = scope_pass(&mut session, ranks, semantic)?;
    let scope_ms = elapsed(start);
    let start = Instant::now();
    let scores = rank_and_fuse(vault, config, &session, ranks)?;
    let rank_fuse_ms = elapsed(start);
    let start = Instant::now();
    let step = install_first_page(vault, scope, session, scores, notes)?;
    let pages = step.count.value as usize;
    // Include the same 50-row response hydration/conversion used by History.
    let items = step
        .items
        .iter()
        .map(|hit| {
            history_entry_to_search_entry(
                &hit.visit,
                hit.score,
                hit.reason.as_deref().unwrap_or("lexical"),
            )
        })
        .collect::<Vec<_>>();
    let ids = items.iter().map(|hit| hit.history_id).collect();
    let first_page_ms = elapsed(start);
    let total_ms = elapsed(total);
    release_cursor(vault, step.cursor.as_deref())?;
    Ok((
        Probe {
            query: request.query.clone(),
            lexical_ms,
            semantic_ms,
            scope_ms,
            rank_fuse_ms,
            first_page_ms,
            total_ms,
            public_ms: 0.,
            pages,
            public_items_match: false,
        },
        ids,
    ))
}

#[derive(Serialize)]
struct Memory {
    current_bytes: u64,
    peak_bytes: u64,
    method: &'static str,
}
fn memory() -> Memory {
    #[cfg(target_os = "macos")]
    {
        let mut usage = unsafe { std::mem::zeroed::<libc::rusage_info_v4>() };
        // SAFETY: a valid v4 output buffer and the current process id.
        assert_eq!(
            unsafe {
                libc::proc_pid_rusage(
                    libc::getpid(),
                    libc::RUSAGE_INFO_V4,
                    (&mut usage as *mut libc::rusage_info_v4).cast(),
                )
            },
            0
        );
        Memory {
            current_bytes: usage.ri_phys_footprint,
            peak_bytes: usage.ri_lifetime_max_phys_footprint,
            method: "proc_pid_rusage v4 physical footprint, process lifetime peak",
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
        assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) }, 0);
        Memory {
            current_bytes: usage.ru_maxrss as u64 * 1024,
            peak_bytes: usage.ru_maxrss as u64 * 1024,
            method: "getrusage process max RSS (not current RSS)",
        }
    }
    #[cfg(not(unix))]
    {
        Memory { current_bytes: 0, peak_bytes: 0, method: "unavailable on this host" }
    }
}
fn append(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    writeln!(file)?;
    Ok(())
}

#[tokio::test]
#[ignore = "opt-in semantic stage benchmark; use --release --ignored --nocapture"]
async fn semantic_stage_benchmark() -> Result<()> {
    if std::env::var("PATHKEEP_SEMANTIC_STAGE_BENCH").as_deref() != Ok("1") {
        eprintln!("SKIP: set PATHKEEP_SEMANTIC_STAGE_BENCH=1");
        return Ok(());
    }
    let visits = std::env::var("PATHKEEP_SEMANTIC_VISITS")
        .unwrap_or_else(|_| "264388".into())
        .parse::<i64>()?;
    ensure!(visits > 0, "PATHKEEP_SEMANTIC_VISITS must be positive");
    let out = std::env::var_os("PATHKEEP_SEMANTIC_OUT")
        .context("set PATHKEEP_SEMANTIC_OUT to a JSONL file")?;
    let out = Path::new(&out);
    let provider = AiProviderRuntime {
        config: crate::AiProviderConfig {
            id: "semantic-stage-bench".into(),
            name: "Benchmark in-process provider".into(),
            purpose: crate::AiProviderPurpose::Embedding,
            enabled: true,
            base_url: Some("http://127.0.0.1:1/v1".into()),
            default_model: "stage-unit".into(),
            dimensions: Some(DIM as u32),
            ..Default::default()
        },
        api_key: None,
    };
    let mut config = AppConfig::default();
    config.ai.embedding_providers = vec![provider.config.clone()];
    config.ai.embedding_provider_id = Some(provider.config.id.clone());
    let vault = Vault::open_temp()?.with_config(config.clone());
    let paths = vault.paths();
    let seed_start = Instant::now();
    seed(&vault, visits, &provider, DIM)?;
    println!(
        "\nSemantic stages: visits={visits}, pages={}, dim={DIM}, queries={QUERIES}, plaintext real Vault, seed {:.2}s",
        (visits + 1) / 2,
        seed_start.elapsed().as_secs_f64()
    );
    println!(
        "Stage queries use precomputed vectors. Public semantic_search_history uses the existing cfg(test) external embedding stub; no inference or transport measured. Warm OS cache; no cache flushing. Peak includes archive/index seeding and all 40 searches; separate process per visit count."
    );
    ensure!(crate::semantic_index::load_state(&vault)?.chosen_complete, "ledger is not complete");
    let mut probes = Vec::new();
    for number in 0..QUERIES {
        let request = AiSearchRequest {
            query: format!("topic{number}"),
            limit: Some(50),
            ..Default::default()
        };
        let vector = embed_query(paths, &provider, &request.query, EmbeddingRole::Query).await?;
        let (mut probe, ids) = measured_page(&vault, paths, &config, &provider, &request, &vector)?;
        let start = Instant::now();
        let public =
            crate::semantic_search_history(&vault, paths, &config, None, Some(&provider), &request)
                .await?;
        probe.public_ms = elapsed(start);
        ensure!(
            probe.pages == ((visits + 1) / 2) as usize,
            "semantic search lost archive coverage"
        );
        release_cursor(&vault, public.next_cursor.as_deref())?;
        ensure!(public.note_codes.is_empty(), "public search degraded: {:?}", public.note_codes);
        ensure!(public.total == probe.pages, "public result count differs");
        probe.public_items_match =
            ids == public.items.iter().map(|item| item.history_id).collect::<Vec<_>>();
        ensure!(probe.public_items_match, "public ranking differs");
        append(out, &serde_json::json!({"kind":"probe","visits":visits,"dim":DIM,"probe":probe}))?;
        probes.push(probe);
    }
    println!("| stage | median ms | max ms |\n|---|---:|---:|");
    let mut summary = serde_json::Map::new();
    for (name, select) in [
        ("lexical ranks + session", (|p: &Probe| p.lexical_ms) as fn(&Probe) -> f64),
        ("semantic scores + position map", |p: &Probe| p.semantic_ms),
        ("scope pass", |p: &Probe| p.scope_ms),
        ("rank + fuse + stars", |p: &Probe| p.rank_fuse_ms),
        ("install + first page", |p: &Probe| p.first_page_ms),
        ("total (embedding excluded)", |p: &Probe| p.total_ms),
        ("public entry (test stub embedding)", |p: &Probe| p.public_ms),
    ] {
        let mut samples = probes.iter().map(select).collect::<Vec<_>>();
        samples.sort_by(f64::total_cmp);
        let median = (samples[QUERIES / 2 - 1] + samples[QUERIES / 2]) / 2.;
        let max = *samples.last().unwrap();
        println!("| {name} | {median:.2} | {max:.2} |");
        summary.insert(name.into(), serde_json::json!({"median_ms":median,"max_ms":max}));
    }
    let memory = memory();
    println!(
        "Physical footprint: current {:.2} MB; peak {:.2} MB ({})",
        memory.current_bytes as f64 / 1e6,
        memory.peak_bytes as f64 / 1e6,
        memory.method
    );
    append(
        out,
        &serde_json::json!({"kind":"summary","visits":visits,"pages":(visits+1)/2,"dim":DIM,"queries":QUERIES,"stages":summary,"memory":memory,"embedding":"precomputed for stages; existing cfg(test) stub for public cross-check"}),
    )?;
    Ok(())
}
