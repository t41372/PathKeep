//! Owner-scoped conversation embedding work. The current vector generation remains available
//! during a rebuild. Provider resolution and all provider calls require live device consent.
mod pages;
mod removal;
use super::{
    Empty, Refusal,
    backups::{self, Events},
    preview,
};
use crate::{
    VaultSession,
    context::{selected_embedding_provider_runtime, tokio_runtime},
};
use anyhow::{Context, Result, ensure};
pub(super) use pages::search;
pub use removal::{Confirm, free, preview as preview_free};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Condvar, LazyLock, Mutex},
    time::{Duration, Instant},
};
use vault_core::AiRefusal;
use vault_core::{
    AiCapability, Vault,
    ai::{conversation_vectors as vectors, conversations::consent, select_embedding_provider},
};
use vault_store::CancelToken;

const EVENT: &str = "pathkeep://agent-semantic-index-changed";
static JOBS: LazyLock<Mutex<HashMap<PathBuf, Arc<Job>>>> = LazyLock::new(Default::default);
struct Job {
    id: String,
    owner: std::sync::Weak<vault_store::Store>,
    cancel: CancelToken,
    wake: Condvar,
    state: Mutex<State>,
}
struct State {
    phase: &'static str,
    passages: u64,
    vectors: u64,
    error: Option<&'static str>,
}
impl Job {
    fn snapshot(&self) -> Value {
        let s = self.state.lock().unwrap();
        json!({"jobId":self.id,"state":s.phase,"passages":s.passages,"vectors":s.vectors,"error":s.error})
    }
    fn check(&self, v: &Vault) -> Result<()> {
        self.cancel.check()?;
        ensure!(v.store().is_open(), "archive-replaced");
        let mut state = self.state.lock().unwrap();
        while state.phase == "paused" {
            drop(state);
            self.cancel.check()?;
            consent(v, AiCapability::SemanticIndex)?;
            state = self.state.lock().unwrap();
            if state.phase == "paused" {
                state = self.wake.wait_timeout(state, Duration::from_millis(200)).unwrap().0;
            }
        }
        drop(state);
        Ok(())
    }
}
fn job(v: &Vault) -> Option<Arc<Job>> {
    JOBS.lock()
        .unwrap()
        .get(&v.paths().app_root)
        .filter(|j| j.owner.ptr_eq(&Arc::downgrade(v.store())))
        .cloned()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Control {
    action: Action,
    job_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Build,
    Pause,
    Resume,
    Cancel,
}
fn chosen(cfg: &vault_core::AppConfig) -> Option<&vault_core::AiProviderConfig> {
    let id = cfg.ai.embedding_provider_id.as_deref()?;
    cfg.ai
        .embedding_providers
        .iter()
        .find(|p| p.id == id && p.enabled && p.purpose == vault_core::AiProviderPurpose::Embedding)
}
pub fn status(s: &VaultSession, _: Empty) -> Result<Value> {
    let (v, here) = backups::admitted(s, true)?;
    let kept = vectors::kept_index(v.agent_history()?.as_ref())?.bytes.to_string();
    if !here.agent_viewer.build_semantic_index {
        return Ok(json!({"enabled":false,"keptBytes":kept,"serving":null,"job":null}));
    }
    if let Err(reason) =
        vault_core::ensure_ai_capability_enabled(&here, AiCapability::SemanticIndex)
    {
        return Ok(
            json!({"enabled":false,"keptBytes":kept,"serving":null,"job":null,"reason":reason.code()}),
        );
    }
    let cfg = consent(&v, AiCapability::SemanticIndex)?;
    let Some(provider) = chosen(&cfg) else {
        return Ok(
            json!({"enabled":true,"keptBytes":kept,"serving":null,"stale":true,"job":job(&v).map(|j|j.snapshot()),"reason":"embedding-provider-unavailable"}),
        );
    };
    let serving =
        vectors::current(v.agent_history()?.as_ref())?.filter(|g| g.matches_config(provider));
    let archive = v.agent_history()?;
    let stale = match &serving {
        Some(g) => {
            g.projection != archive.projection_generation()?
                || g.marks != archive.search_marks_generation()?
        }
        None => true,
    };
    Ok(
        json!({"enabled":true,"keptBytes":kept,"serving":serving,"stale":stale,"job":job(&v).map(|j|j.snapshot())}),
    )
}
pub fn control(s: &VaultSession, q: Control, events: Events) -> Result<Value> {
    let (v, _) = backups::admitted(s, false)?;
    consent(&v, AiCapability::SemanticIndex)?;
    if matches!(q.action, Action::Build) {
        return start(&v, events, true);
    }
    let j = job(&v).filter(|j| Some(&j.id) == q.job_id.as_ref()).ok_or(Refusal::Missing)?;
    {
        let mut state = j.state.lock().unwrap();
        state.phase = match (&q.action, state.phase) {
            (Action::Pause, "running") => "paused",
            (Action::Resume, "paused") => "running",
            (Action::Cancel, "running" | "paused") => {
                j.cancel.cancel();
                "cancelling"
            }
            _ => return Err(Refusal::ActionUnavailable.into()),
        };
    }
    j.wake.notify_all();
    let row = j.snapshot();
    events(EVENT, row.clone());
    Ok(row)
}
pub(super) fn preferences_changed(s: &VaultSession, enabled: bool, events: Events) {
    if let Some(v) = s.current_vault().or_else(|| enabled.then(|| s.vault().ok()).flatten()) {
        if enabled {
            update_if_needed(&v, events);
        } else {
            revoke(&v.paths().app_root);
        }
    }
}
pub(crate) fn update_if_needed(v: &Vault, events: Events) {
    if consent(v, AiCapability::SemanticIndex).is_ok()
        && let Err(e) = start(v, events, false)
    {
        log::warn!("Agent semantic index could not start: {e}");
    }
}
fn authorize_provider(vault: &Vault, runtime: &vault_core::AiProviderRuntime) -> Result<()> {
    let cfg = consent(vault, AiCapability::SemanticIndex)?;
    ensure!(
        cfg.ai.embedding_provider_id.as_deref() == Some(&runtime.config.id)
            && cfg.ai.embedding_providers.iter().any(|p| p.enabled
                && p.id == runtime.config.id
                && p.default_model == runtime.config.default_model
                && p.base_url == runtime.config.base_url
                && p.dimensions == runtime.config.dimensions),
        AiRefusal::ProviderChanged
    );
    Ok(())
}
fn start(v: &Vault, events: Events, force: bool) -> Result<Value> {
    let cfg = consent(v, AiCapability::SemanticIndex)?;
    let provider = chosen(&cfg).ok_or(AiRefusal::ProviderMissing)?;
    let archive = v.agent_history()?;
    if !force
        && let Some(g) = vectors::current(v.agent_history()?.as_ref())?
        && g.matches_config(provider)
        && g.projection == archive.projection_generation()?
        && g.marks == archive.search_marks_generation()?
    {
        return Ok(json!({"state":"ready","serving":g}));
    }
    let runtime =
        selected_embedding_provider_runtime(&cfg, None).context(AiRefusal::ProviderMissing)?;
    let gate = vault_core::ControlGate::acquire(v.paths());
    authorize_provider(v, &runtime)?;
    let mut jobs = JOBS.lock().unwrap();
    if let Some(j) = jobs.get(&v.paths().app_root)
        && j.owner.ptr_eq(&Arc::downgrade(v.store()))
        && matches!(j.state.lock().unwrap().phase, "running" | "paused" | "cancelling")
    {
        return Ok(j.snapshot());
    }
    let j = Arc::new(Job {
        id: uuid::Uuid::new_v4().to_string(),
        owner: Arc::downgrade(v.store()),
        cancel: CancelToken::new(),
        wake: Condvar::new(),
        state: Mutex::new(State { phase: "running", passages: 0, vectors: 0, error: None }),
    });
    let row = j.snapshot();
    let foreground = crate::foreground_work::begin(crate::ForegroundWork::AgentSemanticIndex);
    jobs.insert(v.paths().app_root.clone(), j.clone());
    drop(jobs);
    drop(gate);
    let vault = v.clone();
    events(EVENT, row.clone());
    let owner = j.clone();
    let spawn = std::thread::Builder::new().name("agent-semantic-index".into()).spawn(move || {
        let work = || -> Result<()> {
            let check = || -> Result<()> {
                owner.check(&vault)?;
                authorize_provider(&vault, &runtime)?;
                Ok(())
            };
            check()?;
            let engine = engine(&vault, &runtime)?;
            let mut last = Instant::now();
            let rt = tokio_runtime()?;
            loop {
                let generation = rt.block_on(vectors::build(
                    &archive,
                    &runtime,
                    &engine,
                    &owner.cancel,
                    &check,
                    |passages, vectors| {
                        let mut s = owner.state.lock().unwrap();
                        s.passages = passages;
                        s.vectors = vectors;
                        drop(s);
                        if last.elapsed() >= Duration::from_millis(200) {
                            events(EVENT, owner.snapshot());
                            last = Instant::now();
                        }
                    },
                ))?;
                check()?;
                if generation.projection == archive.projection_generation()?
                    && generation.marks == archive.search_marks_generation()?
                {
                    break;
                }
            }
            Ok(())
        };
        let result = work();
        let mut s = owner.state.lock().unwrap();
        s.phase = if owner.cancel.check().is_err() {
            "cancelled"
        } else if result.is_ok() {
            "ready"
        } else {
            "failed"
        };
        if let Err(error) = &result
            && s.phase == "failed"
        {
            s.error = Some(
                error
                    .downcast_ref::<AiRefusal>()
                    .map_or("agent-semantic-build-failed", AiRefusal::code),
            );
        }
        drop(foreground);
        drop(s);
        events(EVENT, owner.snapshot());
    });
    if let Err(e) = spawn {
        j.state.lock().unwrap().phase = "failed";
        return Err(e.into());
    }
    Ok(row)
}

fn engine(
    v: &Vault,
    r: &vault_core::AiProviderRuntime,
) -> Result<vault_core::ai::AnyEmbeddingProvider> {
    select_embedding_provider(v.paths(), r).context(if vault_core::ai::runtime_uses_static(r) {
        AiRefusal::ModelUnavailable
    } else {
        AiRefusal::ProviderMissing
    })
}

/// Current owner task for the shared rail. Actions use the same job identity as Settings.
pub(super) fn task(v: &Vault) -> Option<Value> {
    let j = job(v)?;
    if consent(v, AiCapability::SemanticIndex).is_err() {
        return None;
    }
    let row = j.snapshot();
    let state = row["state"].as_str()?;
    let actions = match state {
        "running" => vec!["pause", "cancel"],
        "paused" => vec!["resume", "cancel"],
        "cancelling" => vec![],
        _ => return None,
    };
    Some(
        json!({"taskId":j.id,"jobId":j.id,"kind":"agent-history","operation":"semantic-index","state":state,"phase":"embedding",
        "progress":{"done":row["passages"].to_string(),"total":null,"unit":"passages"},"actions":actions,
        "controlCommand":"control_agent_semantic_index","attention":false,"holdsQuit":true}),
    )
}

pub(super) fn revoke(root: &std::path::Path) {
    pages::forget(root);
    if let Some(j) = JOBS.lock().unwrap().get(root) {
        j.cancel.cancel();
        j.wake.notify_all();
    }
}
