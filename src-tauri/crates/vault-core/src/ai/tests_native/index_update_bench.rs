//! Measures a warm incremental index job, including admission, claim, publication and cleanup.
//! Fixture creation, the initial fill and post-run verification are outside the two measurements.
//! From src-tauri: PATHKEEP_SEMANTIC_VISITS=100000 PATHKEEP_SEMANTIC_APPEND_VISITS=64
//! cargo test -p vault-core --release --lib semantic_index_update_benchmark -- --ignored --nocapture
use super::fixtures::{SeedVisit, prepared_archive, seed_visits};
use super::{AiRunControl, IndexBackfillLedger, IndexBackfillProgress, embedding_provider};
use crate::ai::{AiProviderRuntime, build_ai_index_with_control, native_ledger};
use crate::{AiIndexRequest, Vault, ai_queue};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use vault_store::{Db, Priority, params_owned as params};

fn size(name: &str, default: i64) -> Result<i64> {
    match std::env::var(name) {
        Ok(value) => value.parse().with_context(|| format!("{name} must be an integer")),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error).context(name.to_owned()),
    }
}

/// Three visits per page, distributed across distinct subjects, paths and titles.
fn seed(vault: &Vault, first: i64, end: i64) {
    const SUBJECTS: [(&str, &str, &str); 5] = [
        ("docs", "storage", "Database recovery guide"),
        ("recipes", "baking", "Sourdough kitchen notes"),
        ("maps", "trails", "Local hiking routes"),
        ("library", "astronomy", "Deep sky observing log"),
        ("music", "concerts", "Chamber music program"),
    ];
    for start in (first..end).step_by(8192) {
        let visits = (start..end.min(start.saturating_add(8192)))
            .map(|id| {
                let page = (id - 1) / 3;
                let (host, section, subject) = SUBJECTS[page as usize % SUBJECTS.len()];
                let url = format!("https://{host}.example/{section}/{page}");
                let title = format!("{subject} {page} | {section}");
                SeedVisit::unix_ms(id, "chrome:Default", &url, Some(&title), 1_700_000_000_000 + id)
            })
            .collect();
        seed_visits(vault, visits);
    }
}

/// Revert one synthetic import so visibility, cached totals and search documents agree.
fn hide_fixture_visits(vault: &Vault, physical: i64, hidden: i64) -> Result<()> {
    let batch = vault.write(Db::Archive, Priority::Batch, move |s| {
        let batch = s.query_one(
            "INSERT INTO import_batches(source_kind,source_path,profile_id,created_at,status,summary_json,visible_items)
             VALUES('takeout','benchmark','chrome:Default','2026-11-18T00:00:00Z','imported','{}',?1) RETURNING id",
            [hidden], |r| Ok(r.get::<i64>(0)?),
        )?;
        let tagged = s.execute(
            "UPDATE visits SET import_batch_id=?1 WHERE id % 50=0 OR id=?2",
            params![batch, physical],
        )?;
        ensure!(tagged == hidden as u64, "unexpected hidden fixture size");
        Ok(batch)
    })?;
    crate::revert_import_batch(vault, batch)?;
    vault.search().cycle()?;
    Ok(())
}

// The worker depends on vault-core. Mirror its small IndexCursorLedger adapter here so
// both builds pay the real queue write for every callback they issue.
struct QueueProbe {
    vault: Vault,
    job: i64,
    started: Instant,
    first_read: OnceLock<Duration>,
    ledger_writes: AtomicU64,
}

impl AiRunControl for QueueProbe {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        // On 9aca8138 the next operation reads IDs; on the fixed build it reads the
        // joined page. The checkpoint is immediately before either read, even with no embeds.
        if detail == "Index build was cancelled before the next chunk." {
            self.first_read.get_or_init(|| self.started.elapsed());
        }
        Ok(())
    }
}

impl IndexBackfillLedger for QueueProbe {
    fn job_id(&self) -> Option<i64> {
        Some(self.job)
    }

    fn record(&self, p: IndexBackfillProgress) -> Result<()> {
        let cursor = ai_queue::IndexBackfillCursor {
            scan: p.scan,
            next_history_id: p.next_history_id,
            embedded_so_far: p.embedded_so_far,
            updated_so_far: p.updated_so_far,
            scan_target: p.scan_target,
            embed_target: p.embed_target,
            ..Default::default()
        };
        let summary = format!("Embedded {} row(s) so far.", p.embedded_so_far);
        let outcome =
            ai_queue::persist_index_cursor(&self.vault, self.job, &cursor, Some(&summary))?;
        ensure!(outcome == ai_queue::CursorPersistOutcome::Persisted, "index cursor lost its job");
        self.ledger_writes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[derive(Serialize)]
struct Measurement {
    pass: &'static str,
    visible_visits: i64,
    hidden_visits: i64,
    indexed_visits: usize,
    checked_visits: u64,
    wall_ms: f64,
    first_visit_read_start_ms: f64,
    // Metrics on both builds exposes checkpoint activity and statement compilations,
    // but neither of these two cumulative transaction counts.
    archive_read_transactions: Option<u64>,
    intelligence_write_transactions: Option<u64>,
    ledger_callback_writes: u64,
    published_stamp_current: bool,
    successor_admitted: bool,
}

async fn run_job(
    vault: &Vault,
    provider: &AiProviderRuntime,
    pass: &'static str,
    visible: i64,
    hidden: i64,
    expected_indexed: i64,
    automatic: bool,
) -> Result<Measurement> {
    let started = Instant::now();
    let job = ai_queue::request_index_job(vault, &AiIndexRequest::default(), automatic)?
        .context("index request was not admitted")?;
    let claimed = ai_queue::claim_next_ai_job(vault, 300)?.context("no claimable index job")?;
    ensure!(claimed.id == job.id, "claimed another job");
    let ai_queue::AiJobPayload::Index { request, cursor, .. } = claimed.payload else {
        anyhow::bail!("claimed a non-index job");
    };
    let probe = Arc::new(QueueProbe {
        vault: vault.clone(),
        job: job.id,
        started,
        first_read: OnceLock::new(),
        ledger_writes: AtomicU64::new(0),
    });
    let report = build_ai_index_with_control(
        vault,
        provider,
        &request,
        Some(probe.clone()),
        cursor.next_history_id,
        cursor.embedded_so_far,
        Some(probe.clone()),
    )
    .await?;
    let elapsed = started.elapsed();
    let first = probe.first_read.get().context("first visit read was never observed")?;

    ensure!(ai_queue::load_ai_job(vault, job.id)?.state == "succeeded", "job did not publish");
    let ai_queue::AiJobPayload::Index { cursor, .. } =
        ai_queue::load_ai_job_payload(vault, job.id)?
    else {
        anyhow::bail!("completed job lost its index payload");
    };
    let scan = cursor.scan.context("completed job has no scan")?;
    ensure!(
        scan.total == (visible + hidden) as u64 && scan.examined == scan.total,
        "scan coverage mismatch"
    );
    ensure!(
        report.indexed_items == expected_indexed as usize && report.updated_items == 0,
        "unexpected embedding outcomes"
    );
    let (pair, model, id) =
        (provider.config.id.clone(), provider.config.default_model.clone(), job.id);
    let (published, successor_admitted) = vault.read(Db::Intelligence, move |s| {
        let published = native_ledger::load_index_ledger(s, &pair, &model)?.source_watermark;
        let successor = s.query_one(
            "SELECT EXISTS(SELECT 1 FROM ai_jobs WHERE job_type='index-build' AND id>?1)",
            [id],
            |r| Ok(r.get::<i64>(0)? != 0),
        )?;
        Ok((published, successor))
    })?;
    let current = native_ledger::current_source_watermark(vault)?;
    let ms = |d: Duration| d.as_micros() as f64 / 1000.0;
    Ok(Measurement {
        pass,
        visible_visits: visible,
        hidden_visits: hidden,
        indexed_visits: report.indexed_items,
        checked_visits: scan.examined,
        wall_ms: ms(elapsed),
        first_visit_read_start_ms: ms(*first),
        archive_read_transactions: None,
        intelligence_write_transactions: None,
        ledger_callback_writes: probe.ledger_writes.load(Ordering::Relaxed),
        published_stamp_current: published == current
            && published == scan.source_watermark
            && published != 0
            && published != -1,
        successor_admitted,
    })
}

#[tokio::test]
#[ignore = "local semantic index update benchmark; use --ignored --nocapture"]
async fn semantic_index_update_benchmark() -> Result<()> {
    let n = size("PATHKEEP_SEMANTIC_VISITS", 1024)?;
    let k = size("PATHKEEP_SEMANTIC_APPEND_VISITS", 64)?;
    ensure!(n > 0 && k >= 0, "need positive visible visits and a nonnegative append count");
    let hidden = 1 + (n - 1) / 49;
    let physical = n.checked_add(hidden).context("initial visit count overflow")?;
    let end =
        physical.checked_add(k).and_then(|v| v.checked_add(1)).context("visit ID overflow")?;
    ensure!(end <= 253_402_300_799_999 - 1_700_000_000_000, "fixture timestamps exceed year 9999");
    let (paths, mut config, db) = prepared_archive();
    config.ai.auto_index_after_backup = true;
    crate::save_config(&crate::ControlGate::acquire(&paths), &config)?;
    let vault = db.vault.with_config(config);
    let provider = embedding_provider(); // cfg(test): deterministic local vectors, no HTTP.
    seed(&vault, 1, physical + 1);
    hide_fixture_visits(&vault, physical, hidden)?;
    let fill = run_job(&vault, &provider, "initial", n, hidden, n, false).await?;
    ensure!(
        fill.published_stamp_current && !fill.successor_admitted,
        "initial fill did not settle"
    );

    println!(); // libtest can leave the test name on the current line.
    for (pass, appended) in [("unchanged", 0), ("after_append", k)] {
        if appended > 0 {
            seed(&vault, physical + 1, end);
        }
        let measurement =
            run_job(&vault, &provider, pass, n + appended, hidden, appended, true).await?;
        println!("{}", serde_json::to_string(&measurement)?);
        ensure!(
            measurement.published_stamp_current && !measurement.successor_admitted,
            "update did not converge"
        );
    }
    Ok(())
}
