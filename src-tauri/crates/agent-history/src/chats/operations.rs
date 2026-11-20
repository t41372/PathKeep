//! Native calls and results, attached to their original records. Only structured
//! edit operations supply changed paths or line counts.
use super::facts::Totals;
use serde_json::{Value, json};
pub(super) fn record(
    facts: &mut Totals,
    kind: &str,
    v: &Value,
    content: &Value,
    codex: bool,
    deep: bool,
) -> Vec<Value> {
    let p = &v["payload"];
    let mut ops = Vec::new();
    let mut call = |name: &str, id: Option<&str>, args: &Value| {
        facts.calls += 1;
        let mut op = json!({"kind":"call","name":name,"callId":id});
        if let Some(edit) = edit(name, args) {
            op["edit"] = edit;
        }
        ops.push(op);
    };
    if !deep && let Some(blocks) = content.as_array() {
        for b in blocks {
            if matches!(b["type"].as_str(), Some("tool_use" | "toolCall"))
                && let Some(name) = b["name"].as_str()
            {
                call(name, b["id"].as_str(), b.get("input").unwrap_or(&b["arguments"]));
            }
        }
    }
    if deep && kind == "tool/call" {
        if let Some(name) = v["data"]["name"].as_str() {
            let args = &v["data"]["arguments"];
            let parsed = args.as_str().and_then(|s| serde_json::from_str::<Value>(s).ok());
            call(name, v["data"]["callId"].as_str(), parsed.as_ref().unwrap_or(args));
        }
    } else if codex
        && matches!(kind, "response_item.function_call" | "response_item.custom_tool_call")
        && let Some(name) = p["name"].as_str()
    {
        let args = p["arguments"].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok());
        call(name, p["call_id"].as_str(), args.as_ref().unwrap_or(&p["input"]));
    }
    if let Some(blocks) = content.as_array() {
        for b in blocks {
            if b["type"] == "tool_result" {
                let denied = b["status"] == "denied" || v["toolUseResult"]["status"] == "denied";
                let failed = denied || b["is_error"] == true;
                facts.failed_calls += i64::from(failed);
                ops.push(json!({"kind":"result","callId":b["tool_use_id"],"status":if denied{"denied"}else if failed{"error"}else{"ok"}}));
            }
        }
    }
    if v["message"]["role"] == "toolResult" {
        let failed = v["message"]["isError"] == true;
        facts.failed_calls += i64::from(failed);
        ops.push(json!({"kind":"result","callId":v["message"]["toolCallId"],"status":if failed{"error"}else{"ok"}}));
    }

    if matches!(
        kind,
        "response_item.function_call_output" | "response_item.custom_tool_call_output"
    ) {
        ops.push(json!({"kind":"result","callId":p["call_id"],"status":null}));
    }
    if deep && kind == "tool/result" {
        let m = &v["data"]["message"];
        let status = match m.get("isError").and_then(Value::as_bool) {
            Some(true) => Some("error"),
            Some(false) => Some("ok"),
            None => None,
        };
        facts.failed_calls += i64::from(status == Some("error"));
        ops.push(json!({"kind":"result","callId":m["toolCallId"],"status":status}));
    }
    if kind == "event_msg.item_completed.CommandExecution" {
        let item = &p["item"];
        let exit = item["exit_code"].as_i64();
        let status = exit.map(|n| if n == 0 { "ok" } else { "error" });
        facts.failed_calls += i64::from(status == Some("error"));
        ops.push(json!({"kind":"result","callId":item.get("call_id").or(item.get("id")),"status":status,"durationMs":item["duration_ms"]}));
    }
    if kind == "event_msg.patch_apply_end" {
        let status = p["success"].as_bool().map(|ok| if ok { "ok" } else { "error" });
        ops.push(json!({"kind":"result","callId":p["call_id"],"status":status}));
    }
    if let Some(blocks) = content.as_array() {
        for b in
            blocks.iter().filter(|b| matches!(b["type"].as_str(), Some("thinking" | "reasoning")))
        {
            if let Some(ms) = b.get("durationMs").or(b.get("duration_ms")).and_then(Value::as_i64) {
                ops.push(json!({"kind":"thinking","durationMs":ms}));
            }
        }
    }
    for op in &mut ops {
        if op["kind"] == "result"
            && let Some(agent) = v.pointer("/toolUseResult/agentId").and_then(Value::as_str)
        {
            op["agentId"] = json!(agent);
        }
        if op["kind"] == "result" && !v["toolUseResult"]["durationMs"].is_null() {
            op["durationMs"] = v["toolUseResult"]["durationMs"].clone();
        }
    }
    ops
}

/// Only explicit lifecycle records produce a banner. Native message text
/// stays with the record and the front end translates the category.
pub(super) fn notice(kind: &str, v: &Value) -> Option<(&'static str, &'static str)> {
    match (kind, v["subtype"].as_str()) {
        ("event_msg.turn_aborted" | "turn/abort", _) => Some(("interrupted", "warning")),
        ("turn/end", _)
            if v.pointer("/data/reason/kind").and_then(Value::as_str) == Some("interrupted") =>
        {
            Some(("interrupted", "warning"))
        }
        ("event_msg.error" | "request/error", _) | (_, Some("api_error")) => {
            Some(("apiError", "error"))
        }
        ("request/retry" | "event_msg.stream_error", _) | (_, Some("api_retry")) => {
            Some(("retry", "info"))
        }
        ("user", _)
            if v.pointer("/message/content")
                .and_then(Value::as_str)
                .is_some_and(|s| s.starts_with("[Request interrupted")) =>
        {
            Some(("interrupted", "warning"))
        }
        _ => None,
    }
}

/// A bounded patch excerpt accompanies the call. Counting consumes all changed
/// lines even when the display preview is clipped.
fn edit(name: &str, args: &Value) -> Option<Value> {
    use similar::{ChangeTag, TextDiff};
    let mut files = std::collections::BTreeSet::new();
    let mut preview = String::new();
    let mut truncated = false;
    let mut line = |prefix: char, s: &str| {
        if preview.len() >= 8192 {
            truncated = true;
            return;
        }
        preview.push(prefix);
        for c in s.chars() {
            if preview.len() + c.len_utf8() > 8192 {
                truncated = true;
                break;
            }
            preview.push(c);
        }
        if !preview.ends_with('\n') {
            preview.push('\n');
        }
    };
    let (mut added, mut removed) = (0_i64, 0_i64);
    if name == "apply_patch" {
        for s in args.as_str()?.lines() {
            for prefix in ["*** Update File: ", "*** Add File: ", "*** Delete File: "] {
                if let Some(path) = s.strip_prefix(prefix) {
                    files.insert(path);
                }
            }
            if let Some(s) = s.strip_prefix('+') {
                added += 1;
                line('+', s);
            }
            if let Some(s) = s.strip_prefix('-') {
                removed += 1;
                line('-', s);
            }
        }
    } else if matches!(name, "Edit" | "edit" | "Write" | "write" | "write_file" | "replace_in_file")
    {
        let path = args["file_path"].as_str().or(args["path"].as_str())?;
        files.insert(path);
        let old = args["old_string"].as_str().or(args["oldText"].as_str());
        let new = args["new_string"].as_str().or(args["newText"].as_str());
        if let Some((old, new)) = old.zip(new) {
            let diff = TextDiff::from_lines(old, new);
            for change in diff.iter_all_changes() {
                match change.tag() {
                    ChangeTag::Delete => {
                        removed += 1;
                        line('-', change.value())
                    }
                    ChangeTag::Insert => {
                        added += 1;
                        line('+', change.value())
                    }
                    ChangeTag::Equal => {}
                }
            }
        } else {
            // A write can replace a pre-existing file of unknown length.
            return Some(
                json!({"files":files,"addedLines":null,"removedLines":null,"diff":null,"truncated":false}),
            );
        }
    } else {
        return None;
    }
    Some(
        json!({"files":files,"addedLines":added,"removedLines":removed,"diff":preview,"truncated":truncated}),
    )
}
