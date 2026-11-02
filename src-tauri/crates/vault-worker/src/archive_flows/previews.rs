//! Responsibilities: fetch link previews after a backup without holding the backup's reply.
//! Not responsible for: backing up, a run's outcome, or the fetch policy and its budgets.
//! Failure modes: a backup's reply waits on the network; two passes fetch the same pages at once;
//! a pass started before the user turned fetching off still fetches; the worker exits idle
//! between a pass being asked for and its thread starting.
//!
//! One pass at a time. A backup that ends while one runs leaves its request, replacing an older
//! waiting one: each pass reads the settings and budgets current when it starts, so a newer
//! request covers what an older one would have fetched. What a pass could not do is said on the
//! run that asked for it.
use super::{
    append_og_image_prefetch_result, append_og_image_refetch_due_result, clamp_budget,
    lock_or_recover, try_prefetch_new_visit_og_images, try_refetch_due_og_images,
};
use crate::job_runtime::{background_epoch, with_worker_epoch, worker_stopped};
use anyhow::{Context, Result};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use vault_core::{BackupReport, Vault};

/// A pass asked for or running: idle exit waits for it from the moment it is asked for.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn active() -> usize {
    ACTIVE.load(Ordering::Acquire)
}

#[derive(Default)]
struct Lane {
    running: bool,
    waiting: Option<(Vault, i64)>,
}

static LANE: Mutex<Lane> = Mutex::new(Lane { running: false, waiting: None });

/// Counts a pass in `ACTIVE` until its thread ends, unwinding included.
struct Active;
impl Active {
    fn new() -> Self {
        ACTIVE.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Asks for a preview pass for the backup `run_id` committed.
pub(super) fn enqueue(vault: &Vault, run_id: i64) -> Result<()> {
    let mut lane = lock_or_recover(&LANE);
    lane.waiting = Some((vault.clone(), run_id));
    if lane.running {
        return Ok(());
    }
    lane.running = true;
    let epoch = background_epoch();
    let active = Active::new();
    let spawned =
        std::thread::Builder::new().name("pathkeep-link-previews".into()).spawn(move || {
            let _active = active;
            with_worker_epoch(epoch, drain);
        });
    if let Err(error) = spawned {
        lane.running = false;
        lane.waiting = None;
        return Err(error).context("starting link previews in the background");
    }
    Ok(())
}

fn drain() {
    loop {
        let (vault, run_id) = {
            let mut lane = lock_or_recover(&LANE);
            match lane.waiting.take().filter(|_| !worker_stopped()) {
                Some(request) => request,
                None => {
                    lane.running = false;
                    lane.waiting = None;
                    return;
                }
            }
        };
        if !vault.store().is_open() {
            continue;
        }
        let mut report = BackupReport::default();
        if let Err(error) = fetch(&vault, &mut report) {
            append_og_image_prefetch_result(&mut report, Err(error));
        }
        if !worker_stopped()
            && vault.store().is_open()
            && !report.warning_details.is_empty()
            && let Err(error) =
                vault_core::append_run_warnings(&vault, run_id, &report.warning_details)
        {
            eprintln!("PathKeep could not save link-preview warnings for run {run_id}: {error:#}");
        }
    }
}

/// Fetches again the previews whose retry time came, then the new visits' pages, each within
/// the budgets the settings give now.
fn fetch(vault: &Vault, report: &mut BackupReport) -> Result<()> {
    let current = current_settings(vault)?;
    let budget = clamp_budget(current.config().og_image.daily_refetch_budget);
    append_og_image_refetch_due_result(report, try_refetch_due_og_images(&current, budget));
    if worker_stopped() || !vault.store().is_open() {
        return Ok(());
    }
    let current = current_settings(vault)?;
    let budget = clamp_budget(current.config().og_image.new_visit_prefetch_budget);
    append_og_image_prefetch_result(report, try_prefetch_new_visit_og_images(&current, budget));
    Ok(())
}

/// The link-preview settings as saved now, not as they were when the backup started: the user
/// may have turned fetching off meanwhile. A missing settings file is an error, not the defaults,
/// which could turn fetching on.
fn current_settings(vault: &Vault) -> Result<Vault> {
    let text = std::fs::read_to_string(&vault.paths().config_path)
        .context("reading the link-preview settings")?;
    let saved: vault_core::AppConfig =
        serde_json::from_str(&text).context("reading the link-preview settings")?;
    let mut config = vault.config().clone();
    config.og_image = saved.og_image;
    Ok(vault.with_config(config))
}
