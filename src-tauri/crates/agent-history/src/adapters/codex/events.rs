//! Responsibilities: project readable Codex event fields without changing native payloads.
//! Not responsible for: naming threads, discovery or resolving rollout lineage.
use super::string;
use crate::read_model::{PartKind, RecordPart, parts};
use crate::{NativeEvent, adapters::common};
use serde_json::Value;

pub(crate) fn native_event(record: Value, ordinal: u64) -> NativeEvent {
    let outer = record["type"].as_str().unwrap_or("unknown");
    let payload = &record["payload"];
    let inner = payload["type"].as_str();
    let completed = outer == "event_msg" && inner == Some("item_completed");
    let content = if completed { &payload["item"] } else { payload };
    let native_type = content["type"].as_str().unwrap_or(outer);
    let kind = match (inner, completed) {
        (Some(inner), true) => format!("{outer}.{inner}.{native_type}"),
        (Some(inner), false) => format!("{outer}.{inner}"),
        (None, _) => outer.to_owned(),
    };
    let role = match native_type {
        "message" => string(content, "role"),
        "UserMessage" | "user_message" => Some("user".into()),
        "AgentMessage"
        | "agent_message"
        | "Reasoning"
        | "reasoning"
        | "agent_reasoning"
        | "agent_reasoning_raw_content"
        | "function_call"
        | "custom_tool_call" => Some("assistant".into()),
        "FunctionCallOutput"
        | "CommandExecution"
        | "McpToolCall"
        | "function_call_output"
        | "custom_tool_call_output"
        | "mcp_tool_call_end" => Some("tool".into()),
        _ => None,
    };
    let text = readable_text(native_type, content);
    NativeEvent {
        references: Vec::new(),
        ordinal,
        native_id: string(content, "id"),
        parent_id: string(content, "call_id"),
        kind,
        role,
        timestamp: event_time(&record),
        text,
        payload: record,
    }
}

fn readable_text(kind: &str, value: &Value) -> String {
    match kind {
        "message" | "agent_message" | "UserMessage" | "AgentMessage" => {
            if value["content"].is_null() {
                common::text(&value["message"])
            } else {
                common::text(&value["content"])
            }
        }
        "user_message" => common::text(&value["message"]),
        "function_call" | "custom_tool_call" => {
            common::text(&value["arguments"]) + &common::text(&value["input"])
        }
        "function_call_output" | "custom_tool_call_output" | "FunctionCallOutput" => {
            common::text(&value["output"])
        }
        "CommandExecution" => common::text(&value["aggregated_output"]),
        "McpToolCall" => common::text(&value["result"]["content"]),
        "mcp_tool_call_end" => {
            if value["result"]["Err"].is_null() {
                common::text(&value["result"]["Ok"]["content"])
            } else {
                common::text(&value["result"]["Err"])
            }
        }
        "agent_reasoning" | "agent_reasoning_raw_content" => common::text(&value["text"]),
        "reasoning" => common::text(&value["summary"]),
        "Reasoning" => common::text(&value["summary_text"]),
        "Plan" => common::text(&value["text"]),
        _ => String::new(),
    }
}

pub(super) fn event_time(record: &Value) -> Option<i64> {
    common::timestamp(&record["timestamp"])
        .or_else(|| common::timestamp(&record["payload"]["timestamp"]))
        .or_else(|| common::timestamp(&record["payload"]["completed_at_ms"]))
}

pub(crate) fn parts(event: &NativeEvent) -> Vec<RecordPart> {
    let payload = &event.payload["payload"];
    let item = if event.payload["type"] == "event_msg" && payload["type"] == "item_completed" {
        &payload["item"]
    } else {
        payload
    };
    match item["type"].as_str() {
        Some("function_call" | "custom_tool_call") => {
            let mut result = Vec::new();
            parts::call(
                &mut result,
                item,
                if item["type"] == "function_call" { "arguments" } else { "input" },
            );
            result
        }
        Some("reasoning" | "Reasoning" | "agent_reasoning" | "agent_reasoning_raw_content") => {
            parts::single(PartKind::Thinking, &event.text)
        }
        Some("message" | "agent_message" | "UserMessage" | "AgentMessage")
            if !item["content"].is_null() =>
        {
            parts::content(&item["content"], PartKind::Text)
        }
        _ => parts::single(
            if event.role.as_deref() == Some("tool") {
                PartKind::CallResult
            } else {
                PartKind::Text
            },
            &event.text,
        ),
    }
}
