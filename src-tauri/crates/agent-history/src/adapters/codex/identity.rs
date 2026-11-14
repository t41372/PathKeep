//! Responsibilities: name Codex threads and their physical rollout references.
//! Not responsible for: resolving another thread or reading a rollout from disk.
use crate::{SessionMetadata, adapters::identity::ConversationKey};
use serde_json::Value;

pub(super) fn key(session: &SessionMetadata, first: &Value) -> ConversationKey {
    // Reverted rollouts have their own session_meta timestamps. Only the thread
    // ID repeats across them; using any rollout time would split the user's state.
    let header = if first["type"] == "session_meta" { &first["payload"] } else { &Value::Null };
    let native = if session.native_id.is_empty() {
        header["id"].as_str().unwrap_or_default().to_owned()
    } else {
        session.native_id.clone()
    };
    let mut key = ConversationKey::new(native, String::new());
    key.relation.parent_native_id = session.parent_session_id.clone().or_else(|| {
        header["parent_thread_id"]
            .as_str()
            .or_else(|| header["source"]["subagent"]["thread_spawn"]["parent_thread_id"].as_str())
            .map(str::to_owned)
    });
    let base = &first["payload"]["history_base"];
    key.relation.base_native_id = base["thread_id"].as_str().map(str::to_owned);
    key.relation.base_end_ordinal = base["end_ordinal_exclusive"].as_u64();
    key.relation.base_end_byte = base["end_byte_offset"].as_u64();
    if key.relation.parent_native_id.is_none() {
        key.relation.forked_from_native_id = session
            .forked_from_session_id
            .clone()
            .or_else(|| header["forked_from_id"].as_str().map(str::to_owned));
    }
    key
}

pub(super) fn filename_time(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?.strip_prefix("rollout-")?;
    name.get(..19)
}

/// history_base names an immutable rollout ID, which may differ from its thread ID.
pub(crate) fn rollout_id(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?.strip_prefix("rollout-")?;
    let stem = name.strip_suffix(".zst").unwrap_or(name).strip_suffix(".jsonl")?;
    if let Some((_, rollout)) = stem.rsplit_once('_') {
        return Some(rollout);
    }
    stem.get(stem.len().checked_sub(36)?..)
}
