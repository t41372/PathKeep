//! Responsibilities: time a full Insights rebuild of an E2E-sized archive, stage by stage.
//! Not responsible for: checking what the build derives (`intelligence_turso.rs` does).
//! Failure modes: a stage that commits or checkpoints per small piece of work turns a
//! 22k-visit rebuild into a minute; the per-phase times and checkpoint counts show where.

use std::time::Instant;
use vault_core::{CoreIntelligenceRebuildRequest, Vault, run_core_intelligence_with_progress};
use vault_store::{Db, Priority, params};

/// Three browser profiles shaped like the E2E fixture: 8,775, 8,675 and 4,205 visits spread
/// over two years, one in ten a search, most following the previous visit.
fn seed(vault: &Vault) {
    vault.store().write(Db::Archive, Priority::Batch, |s| {
        let mut visit_id = 0_i64;
        for (profile, visits) in [(1_i64, 8_775_i64), (2, 8_675), (3, 4_205)] {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,enabled,profile_key) VALUES(?1,'chrome',?2,'/fixture','',1,?3)", params![profile, format!("P{profile}"), format!("chrome:P{profile}")])?;
            let mut page = s.prepare("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3) ON CONFLICT(id) DO NOTHING")?;
            let mut url = s.prepare("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(?1,?2,?3,?4,0,'',?5,'',?6,0,?1) ON CONFLICT(id) DO UPDATE SET last_visit_ms=excluded.last_visit_ms")?;
            let mut visit = s.prepare("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id,from_visit,transition_type) VALUES(?1,?2,?3,?4,'',?5,0,?6,?7)")?;
            let span = 730 * 86_400_000_i64;
            for n in 0..visits {
                visit_id += 1;
                let page_id = 1 + n % 1_500;
                let (address, domain) = if n % 10 == 0 {
                    (format!("https://www.google.com/search?q=topic+{}", n % 700), "google.com".to_owned())
                } else {
                    let site = page_id % 120;
                    (format!("https://site{site}.example/page/{page_id}"), format!("site{site}.example"))
                };
                let page_key = if n % 10 == 0 { 10_000 + n % 700 } else { page_id };
                page.execute(params![page_key, address.as_str(), domain])?;
                let url_id = profile * 100_000 + page_key;
                let time = 1_700_000_000_000 + n * span / visits;
                url.execute(params![url_id, page_key, address.as_str(), format!("Page {page_key}"), time, profile])?;
                let from = if n % 10 == 0 { None } else { Some(visit_id - 1) };
                visit.execute(params![visit_id, url_id, n.to_string(), time, profile, from, if n % 7 == 0 { 1 } else { 0 }])?;
            }
            s.execute("INSERT INTO archive_totals(name,value) VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET value=excluded.value", params![format!("sourceVisits:{profile}"), visits])?;
        }
        s.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits) WHERE name='visits'", ())?;
        s.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM urls) WHERE name='urls'", ())?;
        Ok(())
    })
    .unwrap();
}

#[test]
#[ignore = "timing; run with --ignored --nocapture"]
fn full_rebuild_of_an_e2e_sized_archive() {
    // PATHKEEP_BENCH_ROOT / PATHKEEP_BENCH_PASSWORD time an existing data folder instead,
    // such as the one the E2E app built, to tell its data apart from its environment.
    let vault = match std::env::var("PATHKEEP_BENCH_ROOT") {
        Ok(root) => {
            let paths = vault_core::project_paths_with_root(std::path::Path::new(&root));
            let config = vault_core::load_config(&paths).unwrap();
            let password = std::env::var("PATHKEEP_BENCH_PASSWORD").unwrap();
            Vault::open_password(paths, config, &password, None).unwrap().0
        }
        Err(_) => {
            let vault = Vault::open_temp_with_key(Some([71; 32])).unwrap();
            seed(&vault);
            vault
        }
    };
    let started = Instant::now();
    let mut phase = String::new();
    let mut phase_started = Instant::now();
    let mut phases = Vec::<(String, f64)>::new();
    run_core_intelligence_with_progress(
        &vault,
        &CoreIntelligenceRebuildRequest { full_rebuild: true, ..Default::default() },
        |p| {
            if phase != p.phase {
                if !phase.is_empty() {
                    phases.push((phase.clone(), phase_started.elapsed().as_secs_f64()));
                }
                phase = p.phase;
                phase_started = Instant::now();
            }
            Ok(())
        },
    )
    .unwrap();
    phases.push((phase, phase_started.elapsed().as_secs_f64()));
    for (name, seconds) in &phases {
        println!("rebuild_phase {name} {seconds:.2}s");
    }
    println!(
        "rebuild_total {:.2}s intelligence_metrics={:?}",
        started.elapsed().as_secs_f64(),
        vault.store().metrics(Db::Intelligence)
    );
}
