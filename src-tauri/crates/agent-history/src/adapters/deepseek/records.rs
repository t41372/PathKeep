//! Responsibilities: validate and render the known DeepSeek record vocabulary.
//! Not responsible for: lifecycle state, discovery or granting reads from text spill notices.
use crate::adapters::common;
use serde_json::Value;

pub(super) fn message<'a>(payload: &'a Value, kind: &str) -> Option<&'a Value> {
    match kind {
        "user/message" => payload.get("data"),
        "system/message" | "developer/message" | "assistant/message" | "tool/result" => {
            payload.pointer("/data/message")
        }
        _ => None,
    }
}

pub(super) fn role_for_event(kind: &str) -> Option<&'static str> {
    match kind {
        "user/message" => Some("user"),
        "system/message" => Some("system"),
        "developer/message" => Some("developer"),
        "assistant/message" => Some("assistant"),
        "tool/result" => Some("tool"),
        _ => None,
    }
}

fn nonempty_string(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.is_empty())
}

/// Validate only fields this projection consumes. Native lifecycle and surface
/// replay validation remains the harness's responsibility, not this query reader's.
pub(super) fn valid_record(payload: &Value, kind: &str) -> bool {
    if !nonempty_string(&payload["type"])
        || !payload["data"].is_object()
        || payload["time"].as_i64().is_none()
    {
        return false;
    }
    let data = &payload["data"];
    if let Some(role) = role_for_event(kind) {
        let Some(message) = message(payload, kind) else { return false };
        if !nonempty_string(&message["id"])
            || message["role"] != role
            || !nonempty_string(&message["source"]["kind"])
        {
            return false;
        }
        let Some(content) = message["content"].as_array() else { return false };
        if !content.iter().all(valid_content_block) {
            return false;
        }
        let surface = &payload["surfaceOp"];
        if surface != "append"
            && !(surface["op"] == "replace"
                && surface["startSeq"].as_u64().is_some()
                && surface["endSeq"].as_u64().is_some())
        {
            return false;
        }
        if kind == "assistant/message"
            && (!data["stream"].is_array()
                || !nonempty_string(&message["source"]["provider"])
                || !nonempty_string(&message["source"]["model"]))
        {
            return false;
        }
        if kind == "tool/result"
            && (!nonempty_string(&message["toolCallId"])
                || message["source"]["callId"] != message["toolCallId"])
        {
            return false;
        }
    }
    match kind {
        "tool/call" => {
            nonempty_string(&data["callId"])
                && nonempty_string(&data["name"])
                && data["arguments"].is_string()
        }
        "assistant/attempt" => data["stream"].is_array(),
        "session/title" => data["title"].is_string(),
        _ => true,
    }
}

fn valid_content_block(block: &Value) -> bool {
    match block["type"].as_str() {
        Some("text" | "reasoning") => block["text"].is_string(),
        Some("tool-call") => {
            nonempty_string(&block["id"])
                && nonempty_string(&block["name"])
                && block["arguments"].is_string()
        }
        Some("image" | "file") => block["attachment"].is_object(),
        Some("tool-addition" | "tool-removal") => nonempty_string(&block["toolName"]),
        Some(kind) => !kind.is_empty(),
        None => false,
    }
}

pub(super) fn native_text(payload: &Value, kind: &str, message: Option<&Value>) -> String {
    let mut pieces = Vec::new();
    let content = message
        .map(|message| &message["content"])
        .or_else(|| (kind == "tool/ptc-dispatch").then_some(&payload["data"]["content"]));
    if let Some(content) = content.and_then(Value::as_array) {
        for block in content {
            if block["type"] == "tool-call" {
                if let Some(name) = block["name"].as_str() {
                    pieces.push(name.to_owned());
                }
                if let Some(arguments) = block["arguments"].as_str() {
                    pieces.push(arguments.to_owned());
                }
            } else {
                let text = common::text(block);
                if !text.is_empty() {
                    pieces.push(text);
                }
            }
        }
    }
    if matches!(kind, "tool/call" | "tool/ptc-dispatch") {
        if let Some(name) = payload["data"]["name"].as_str() {
            pieces.push(name.to_owned());
        }
        if let Some(arguments) = payload["data"].get("arguments") {
            pieces.push(
                arguments.as_str().map(str::to_owned).unwrap_or_else(|| arguments.to_string()),
            );
        }
    }
    pieces.join("\n")
}

/// Recognize the upstream footer convention conservatively, without extracting
/// its path. Tool output can imitate this spelling; it never proves authority.
pub(super) fn content_has_spill_notice(content: &Value) -> bool {
    content.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| match block["type"].as_str() {
            Some("text") => block["text"].as_str().is_some_and(has_spill_notice),
            Some("tool-result") => content_has_spill_notice(&block["content"]),
            _ => false,
        })
    })
}

pub(super) fn has_spill_notice(text: &str) -> bool {
    let footer = text.rsplit("\n\n").next().unwrap_or(text);
    footer.ends_with(')')
        && footer.contains(" Full formatted result stored at: ")
        && (footer.starts_with("(Omitted ")
            || footer.starts_with("(More bytes were omitted.")
            || footer.starts_with("( Full formatted")
            || footer.starts_with("( Omitted "))
}

// The pinned upstream generated vocabulary. Recognition here permits evidence
// projection; it does not claim to implement the harness's native replay engine.
pub(super) const KNOWN_EVENTS: &[&str] = &[
    "agent-preset/selected",
    "agent/inbox/spliced",
    "approval/asked",
    "approval/decided",
    "approval/policy",
    "assistant/attempt",
    "assistant/message",
    "command/done",
    "command/run",
    "compaction/end",
    "compaction/prune",
    "compaction/start",
    "compaction/summary",
    "deliverables/presented",
    "developer/message",
    "feedback/message-delete",
    "feedback/message-put",
    "feedback/record",
    "goal/change",
    "hook/invoked",
    "hook/result",
    "image/offload",
    "job/registered",
    "job/settled",
    "llm/retry",
    "llm/retry-started",
    "model/selection",
    "permission/preset",
    "plan/mode",
    "request/context",
    "request/header",
    "sandbox/mode",
    "schedule/change",
    "session-log-deepseek/delivery-accepted",
    "session/end-seed",
    "session/title",
    "session/title-llm-request",
    "step/end",
    "step/start",
    "subagent/catalog",
    "subagent/descriptor",
    "subagent/model-selection-policy",
    "system/message",
    "team/member",
    "team/message/delivered",
    "team/message/queued",
    "team/mode",
    "team/task",
    "todo/write",
    "tool-workflow/agent-end",
    "tool-workflow/agent-start",
    "tool-workflow/run-end",
    "tool-workflow/run-start",
    "tool/call",
    "tool/ptc-dispatch",
    "tool/ptc-dispatch-start",
    "tool/result",
    "turn/end",
    "turn/start",
    "user/message",
    "web/deepseek-search-llm-request",
    "working-directory/change",
    "workspace/changes",
];
