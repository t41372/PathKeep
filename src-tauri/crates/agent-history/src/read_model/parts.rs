//! Classifies readable native content for a reader. The public event's legacy
//! searchable text remains the export contract; raw payloads preserve opaque fields.
use super::RecordContext;
use crate::{NativeEvent, adapters::common};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Allows a reader to collapse a call or reasoning without hiding the reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartKind {
    Text,
    Thinking,
    CallInput,
    CallResult,
}
/// One independently displayable piece of a native record, in source order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordPart {
    pub kind: PartKind,
    pub text: String,
}

pub(super) fn render(context: &RecordContext, event: &NativeEvent) -> Vec<RecordPart> {
    use crate::adapters::{claude, codex, deepseek, pi, session_json};
    match context {
        RecordContext::Claude => claude::parts(event),
        RecordContext::Codex => codex::parts(event),
        RecordContext::Pi { .. } => pi::parts(event),
        RecordContext::Deepseek { version } => deepseek::parts(event, *version),
        RecordContext::Continue => session_json::parts(event, true),
        RecordContext::Cline | RecordContext::RooCode => session_json::parts(event, false),
    }
}

/// The caller selects a documented native content slot. Opaque metadata never enters this walk.
pub(crate) fn content(value: &Value, default: PartKind) -> Vec<RecordPart> {
    use PartKind::*;
    let mut parts = Vec::new();
    match value {
        Value::Array(blocks) => {
            for block in blocks {
                match block["type"].as_str() {
                    Some("tool_use") => call(&mut parts, block, "input"),
                    Some("toolCall" | "tool-call") => call(&mut parts, block, "arguments"),
                    Some("thinking" | "reasoning" | "reasoning_text") => {
                        push(&mut parts, Thinking, common::text(block))
                    }
                    Some("tool_result" | "tool-result") => {
                        push(&mut parts, CallResult, common::text(&block["content"]))
                    }
                    _ => push(&mut parts, default, common::text(block)),
                }
            }
        }
        Value::String(text) => push(&mut parts, default, text.clone()),
        _ => {}
    }
    parts
}

pub(crate) fn single(kind: PartKind, text: &str) -> Vec<RecordPart> {
    let mut parts = Vec::new();
    push(&mut parts, kind, text.into());
    parts
}
pub(crate) fn call(parts: &mut Vec<RecordPart>, value: &Value, field: &str) {
    let mut text = value["name"].as_str().unwrap_or_default().to_owned();
    if let Some(args) = value.get(field) {
        if !text.is_empty() {
            text.push('\n')
        }
        if let Some(args) = args.as_str() {
            text.push_str(args)
        } else {
            text.push_str(&args.to_string())
        }
    }
    push(parts, PartKind::CallInput, text);
}
pub(crate) fn push(parts: &mut Vec<RecordPart>, kind: PartKind, text: String) {
    if !text.is_empty() {
        parts.push(RecordPart { kind, text })
    }
}
