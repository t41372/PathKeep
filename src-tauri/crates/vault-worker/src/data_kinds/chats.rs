//! Admission and delivery for the two conversation screens. The reader owns
//! paging and accounting; a paused capture policy still permits reading.
use super::{Refusal, backups, delivery};
use crate::VaultSession;
pub use agent_history::Error as ReaderError;
pub use agent_history::chats::{AttachmentQuery, BranchesQuery, ListQuery, RawQuery, WindowQuery};
use agent_history::{Archive, archive::ConversationMark, read_model::JobControl};
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};
use vault_store::CancelToken;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    pub id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "action", deny_unknown_fields)]
pub enum Edit {
    Star { id: String, on: bool },
    Archive { id: String, on: bool },
    Trash { id: String },
    Restore { id: String },
    Rename { id: String, title: String },
}
pub(super) fn admitted(
    session: &VaultSession,
    read: impl FnOnce(&Archive, &vault_core::Vault) -> Result<Value>,
) -> Result<Value> {
    delivery::read(|| {
        let (vault, config) = backups::admitted(session, false)?;
        let archive = vault.agent_history()?;
        archive.set_project_grouping(super::viewer::grouping(&config));
        let value = read(&archive, &vault)?;
        backups::check_reply(&vault, session, false)?;
        Ok(value)
    })
}
fn filtered(
    s: &VaultSession,
    mut q: ListQuery,
    read: impl FnOnce(&Archive, &ListQuery) -> agent_history::Result<Value>,
) -> Result<Value> {
    admitted(s, |a, vault| {
        if q.time_zone.is_none() {
            q.time_zone = Some(vault_core::insights_time_zone(vault)?.shown);
        }
        Ok(read(a, &q)?)
    })
}
pub fn list(s: &VaultSession, q: ListQuery) -> Result<Value> {
    filtered(s, q, Archive::chat_list)
}
pub fn counts(s: &VaultSession, q: ListQuery) -> Result<Value> {
    filtered(s, q, Archive::chat_counts)
}
pub fn header(s: &VaultSession, q: Conversation) -> Result<Value> {
    admitted(s, |a, _| Ok(a.chat_header(&q.id)?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoRequest {
    pub id: String,
    pub show_cost: Option<bool>,
    pub estimate_cost: Option<bool>,
}
pub fn info(s: &VaultSession, q: InfoRequest) -> Result<Value> {
    admitted(s, |a, _| {
        let prefs = a.insight_preferences()?;
        let show = prefs.show_cost && q.show_cost != Some(false);
        let estimate = prefs.estimate_cost && q.estimate_cost != Some(false);
        let mut info = a.chat_info(&q.id)?;
        if !show || !estimate {
            for field in ["own", "total"] {
                if let Some(facts) = info[field].as_object_mut() {
                    facts.remove("estimatedUsd");
                    if !show {
                        facts.remove("recordedUsd");
                    }
                }
            }
            if let Some(fields) = info.as_object_mut() {
                fields.remove("estimatedUsd");
                fields.remove("priceTable");
            }
        }
        Ok(info)
    })
}
pub fn branches(s: &VaultSession, q: BranchesQuery) -> Result<Value> {
    reader(s, q.request_id.as_deref(), |a, j| a.chat_branches(&q, j))
}
pub fn raw(s: &VaultSession, q: RawQuery) -> Result<Value> {
    reader(s, q.request_id.as_deref(), |a, j| a.chat_raw(&q, j))
}
pub fn attachment(s: &VaultSession, q: AttachmentQuery) -> Result<Value> {
    reader(s, q.request_id.as_deref(), |a, j| a.chat_attachment(&q, j))
}
type WindowKey = (PathBuf, String);
static WINDOWS: LazyLock<Mutex<HashMap<WindowKey, CancelToken>>> = LazyLock::new(Default::default);
pub(super) fn cancel_window(root: &std::path::Path, id: &str) {
    if let Some(c) = WINDOWS.lock().unwrap().get(&(root.to_owned(), id.to_owned())) {
        c.cancel();
    }
}
pub fn window(s: &VaultSession, mut q: WindowQuery) -> Result<Value> {
    let request = q.request_id.clone();
    reader(s, request.as_deref(), |a, j| {
        let prefs = a.insight_preferences()?;
        q.show_cost = Some(prefs.show_cost && q.show_cost != Some(false));
        q.estimate_cost = Some(prefs.estimate_cost && q.estimate_cost != Some(false));
        a.chat_window(&q, j)
    })
}
fn reader(
    s: &VaultSession,
    request_id: Option<&str>,
    work: impl FnOnce(&Archive, &JobControl) -> agent_history::Result<Value>,
) -> Result<Value> {
    let job = JobControl::default();
    let key = if let Some(id) = request_id {
        uuid::Uuid::parse_str(id).map_err(|_| Refusal::InvalidRequest)?;
        let key = (vault_core::project_paths()?.app_root, id.to_owned());
        let mut active = WINDOWS.lock().unwrap();
        if active.contains_key(&key) {
            return Err(Refusal::Busy.into());
        }
        active.insert(key.clone(), job.cancel.clone());
        Some(key)
    } else {
        None
    };
    let result = admitted(s, |a, _| {
        let page = work(a, &job)?;
        job.cancel.check().map_err(|_| ReaderError::Cancelled)?;
        Ok(page)
    });
    if let Some(k) = key {
        WINDOWS.lock().unwrap().remove(&k);
    }
    result
}
pub fn edit(s: &VaultSession, q: Edit, events: backups::Events) -> Result<Value> {
    let mut mark = ConversationMark::default();
    let id = match q {
        Edit::Star { id, on } => {
            mark.starred = Some(on);
            id
        }
        Edit::Archive { id, on } => {
            mark.put_away = Some(on);
            id
        }
        Edit::Trash { id } => {
            mark.trash = Some(true);
            id
        }
        Edit::Restore { id } => {
            mark.trash = Some(false);
            id
        }
        Edit::Rename { id, title } => {
            let title = title.trim();
            mark.title = Some((!title.is_empty()).then(|| title.to_owned()));
            id
        }
    };
    let result = admitted(s, |a, _| {
        a.mark_conversation(&id, &mark, true)?;
        Ok(a.chat_header(&id)?)
    })?;
    events(
        "pathkeep://agent-conversations-changed",
        serde_json::json!({"id":id,"viewToken":result["viewToken"]}),
    );
    Ok(result)
}

pub use agent_history::search::ScreenQuery;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindRequest {
    pub id: String,
    pub query: String,
}
pub fn search(s: &VaultSession, mut q: ScreenQuery) -> Result<super::preview::Frames> {
    let (vault, config) = backups::admitted(s, false)?;
    vault.agent_history()?.set_project_grouping(super::viewer::grouping(&config));
    if q.filters.time_zone.is_none() {
        q.filters.time_zone = Some(vault_core::insights_time_zone(&vault)?.shown);
    }
    let frames = super::preview::search_stream(s, vault.agent_history()?, None, q)?;
    backups::check_reply(&vault, s, false)?;
    Ok(frames)
}
pub fn find(s: &VaultSession, q: FindRequest) -> Result<super::preview::Frames> {
    let (vault, config) = backups::admitted(s, false)?;
    vault.agent_history()?.set_project_grouping(super::viewer::grouping(&config));
    let frames = super::preview::search_stream(
        s,
        vault.agent_history()?,
        Some(q.id),
        ScreenQuery::all(q.query),
    )?;
    backups::check_reply(&vault, s, false)?;
    Ok(frames)
}

pub use agent_history::chats::ConversationExport;

/// Check reader admission before opening a system dialog. The write checks again when it returns.
pub fn admit_export(session: &VaultSession, id: &str) -> Result<()> {
    let (vault, _) = backups::admitted(session, false)?;
    vault.agent_history()?.reader_header(id)?;
    Ok(())
}

/// Save only after the native dialog supplies a destination. A failed read leaves any previous file intact.
pub fn export(
    session: &VaultSession,
    mut q: ConversationExport,
    target: &std::path::Path,
) -> Result<Value> {
    use std::io::Write;
    delivery::read(|| {
        let (vault, config) = backups::admitted(session, false)?;
        use agent_history::chats::ExportLocale;
        use vault_core::LanguagePreference;
        q.locale.get_or_insert(match config.preferred_language {
            LanguagePreference::ZhCn => ExportLocale::Simplified,
            LanguagePreference::ZhTw => ExportLocale::Traditional,
            LanguagePreference::System | LanguagePreference::En => ExportLocale::English,
        });
        let folder = std::fs::canonicalize(target.parent().ok_or(Refusal::Destination)?)?;
        let archive = vault.agent_history()?;
        if folder.starts_with(std::fs::canonicalize(archive.root())?)
            || backups::selection::device_roots(&vault, &config.capture_policy)?
                .iter()
                .any(|r| folder.starts_with(&r.path))
        {
            return Err(Refusal::Destination.into());
        }
        vault_core::durable_io::atomic_durable_write_with(target, |out| {
            let mut out = std::io::BufWriter::new(out);
            archive.export_conversation(
                &q,
                &mut out,
                config.agent_viewer.reader_collapse_tool_calls,
                config.agent_viewer.reader_show_thinking,
            )?;
            out.flush()?;
            backups::check_reply(&vault, session, false)?;
            Ok(())
        })?;
        Ok(serde_json::json!({"path":target,"bytes":target.metadata()?.len().to_string()}))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Palette {
    pub text: String,
    pub request_id: Option<String>,
}
pub fn palette(session: &VaultSession, q: Palette) -> Result<Value> {
    reader(session, q.request_id.as_deref(), |a, job| {
        Ok(serde_json::json!({
            "conversations":a.palette_conversations(&q.text,job)?,
            "projects":a.palette_projects(&q.text)?
        }))
    })
}
