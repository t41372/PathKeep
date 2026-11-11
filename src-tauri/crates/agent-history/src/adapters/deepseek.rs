//! Responsibilities: project DeepSeek native log evidence without running the harness.
//! Not responsible for: Zstandard decoding, source repair, native replay, or file reads
//! authorized by conversation text. Raw generations remain independently archived.
//!
//! Format evidence is pinned in research/agent-history/pi-deepseek.md. V4 is the
//! current logical format at upstream fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159;
//! older formats are retained with explicitly limited interpretation.

mod discovery;

use super::common;
use super::{
    Adapter, Artifact, Diagnostic, Harness, NativeEvent, ParseContext, ParseReport, Reference,
    SessionMetadata, SourceRoot,
};
use serde_json::Value;
use std::collections::HashSet;
use std::io::BufRead;

const CURRENT_VERSION: u64 = 4;

/// Keeps the default JSONL/Zstandard provider separate from retired SQLite stores.
pub struct DeepseekAdapter;

impl Adapter for DeepseekAdapter {
    fn harness(&self) -> Harness {
        Harness::Deepseek
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> crate::Result<()> {
        discovery::discover(source, emit, issues)
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> crate::Result<()>,
    ) -> crate::Result<ParseReport> {
        let mut state = Projection::new(context);
        let mut framing_issues = Vec::new();
        let stats = common::records(
            input,
            context,
            &mut |payload, ordinal| state.record(payload, ordinal, emit),
            &mut framing_issues,
        )?;
        for issue in framing_issues {
            common::push_diagnostic(&mut state.issues, issue);
        }
        if stats.unterminated_tail {
            state.issue("deepseek-unterminated-tail", stats.physical_records.checked_sub(1));
        }
        if state.session.is_none() {
            state.issue("deepseek-header-invalid", None);
        }
        Ok(ParseReport {
            session: state.session,
            event_count: stats.emitted_records,
            diagnostics: state.issues,
            references: state.references,
            complete: state.complete && stats.complete,
        })
    }
}

struct Projection<'a> {
    context: &'a ParseContext,
    session: Option<SessionMetadata>,
    version: Option<u64>,
    next_seq: u64,
    complete: bool,
    issues: Vec<Diagnostic>,
    references: Vec<Reference>,
    seen_references: HashSet<String>,
}

impl<'a> Projection<'a> {
    fn new(context: &'a ParseContext) -> Self {
        Self {
            context,
            session: None,
            version: None,
            next_seq: 0,
            complete: true,
            issues: Vec::new(),
            references: Vec::new(),
            seen_references: HashSet::new(),
        }
    }

    fn issue(&mut self, code: &str, ordinal: Option<u64>) {
        self.complete = false;
        common::push_diagnostic(
            &mut self.issues,
            Diagnostic {
                code: code.into(),
                path: Some(self.context.relative_path.clone()),
                ordinal,
                detail: None,
            },
        );
    }

    fn record(
        &mut self,
        payload: Value,
        ordinal: u64,
        emit: &mut dyn FnMut(NativeEvent) -> crate::Result<()>,
    ) -> crate::Result<()> {
        let kind = payload.get("type").and_then(Value::as_str).unwrap_or("unknown").to_owned();
        if ordinal == 0 {
            self.header(&payload);
        } else {
            self.envelope(&payload, ordinal, &kind);
        }
        let interpreted = self.version.is_some_and(|version| version <= CURRENT_VERSION);
        let message = if interpreted { message(&payload, &kind) } else { None };
        let timestamp = payload.get("time").and_then(Value::as_i64);
        if let Some(session) = &mut self.session {
            session.updated_at = session.updated_at.max(timestamp);
            if interpreted && kind == "session/title" {
                session.title =
                    payload.pointer("/data/title").and_then(Value::as_str).map(str::to_owned);
            }
            if let Some(model) =
                message.and_then(|value| value.pointer("/source/model")).and_then(Value::as_str)
            {
                session.model = Some(model.to_owned());
            }
        }
        let native_id = message
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| payload.get("seq").and_then(Value::as_u64).map(|seq| seq.to_string()));
        let role = message.and_then(|_| role_for_event(&kind)).map(str::to_owned);
        let text = if interpreted { native_text(&payload, &kind, message) } else { String::new() };
        if interpreted {
            self.attachments(&payload, &kind, ordinal);
            let spill_content = match kind.as_str() {
                "tool/result" => payload.pointer("/data/message/content"),
                "tool/ptc-dispatch" => payload.pointer("/data/content"),
                _ => None,
            };
            if spill_content.is_some_and(content_has_spill_notice) {
                // Native locators are unauthenticated text, including absolute
                // paths. Report incomplete output instead of granting a read.
                self.issue("deepseek-spill-coverage-unknown", Some(ordinal));
            }
        }
        emit(NativeEvent {
            ordinal,
            native_id,
            parent_id: None,
            kind,
            role,
            timestamp,
            text,
            payload,
        })
    }

    fn header(&mut self, payload: &Value) {
        if payload.get("type").and_then(Value::as_str) != Some("session") {
            self.issue("deepseek-header-invalid", Some(0));
            return;
        }
        let id = payload.get("id").and_then(Value::as_str).filter(|id| !id.is_empty());
        let version = payload.get("version").and_then(Value::as_u64);
        let created_at = payload.get("createdAt").and_then(Value::as_i64).filter(|time| *time >= 0);
        let (Some(id), Some(version), Some(created_at)) = (id, version, created_at) else {
            self.issue("deepseek-header-invalid", Some(0));
            return;
        };
        self.version = Some(version);
        if version != CURRENT_VERSION {
            self.issue("deepseek-format-limited", Some(0));
        } else if payload.get("isSeeded").and_then(Value::as_bool).is_none()
            || payload.get("delegationDepth").and_then(Value::as_u64).is_none()
        {
            self.issue("deepseek-header-invalid", Some(0));
        }
        let parent = payload.get("parentSession").and_then(Value::as_str).map(str::to_owned);
        if let Some(parent) = &parent {
            self.reference("lineage", parent.clone(), 0);
        }
        self.session = Some(SessionMetadata {
            native_id: id.to_owned(),
            project_path: payload.get("cwd").and_then(Value::as_str).map(str::to_owned),
            title: None,
            created_at: Some(created_at),
            updated_at: Some(created_at),
            origin: "unknown".into(),
            harness_version: None,
            parent_session_id: parent,
            model: None,
        });
    }

    fn envelope(&mut self, payload: &Value, ordinal: u64, kind: &str) {
        let seq = payload.get("seq").and_then(Value::as_u64);
        if seq != Some(self.next_seq) {
            self.issue("deepseek-sequence-invalid", Some(ordinal));
        }
        self.next_seq = seq.unwrap_or(self.next_seq).saturating_add(1);
        if self.version == Some(CURRENT_VERSION) && !valid_record(payload, kind) {
            self.issue("deepseek-record-invalid", Some(ordinal));
        }
        if self.version == Some(CURRENT_VERSION)
            && !KNOWN_EVENTS.contains(&kind)
            && payload.get("ignorable").and_then(Value::as_bool) != Some(true)
        {
            self.issue("deepseek-unknown-required-event", Some(ordinal));
        }
    }

    fn reference(&mut self, kind: &str, target: String, ordinal: u64) {
        let key = format!("{kind}:{target}");
        if self.seen_references.contains(&key) {
            return;
        }
        let reference = Reference {
            kind: kind.into(),
            target,
            end_byte_offset: None,
            end_ordinal_exclusive: None,
        };
        if common::push_reference(&mut self.references, reference, &mut self.issues) {
            self.seen_references.insert(key);
        } else {
            self.issue("deepseek-reference-limit", Some(ordinal));
        }
    }

    /// Mirror the upstream exporter's declared content slots; opaque fields grant no reads.
    fn attachments(&mut self, payload: &Value, kind: &str, ordinal: u64) {
        let data = &payload["data"];
        match kind {
            "user/message" | "tool/ptc-dispatch" => self.content(&data["content"], ordinal),
            "system/message" | "developer/message" | "tool/result" | "team/message/queued" => {
                self.content(&data["message"]["content"], ordinal);
            }
            "agent/inbox/spliced" | "session/title-llm-request" => {
                let field = if kind == "agent/inbox/spliced" { "inserted" } else { "messages" };
                if let Some(messages) = data[field].as_array() {
                    for message in messages {
                        self.content(&message["content"], ordinal);
                    }
                }
            }
            "compaction/summary" => {
                self.content(&data["summary"], ordinal);
                self.content(&data["rawOutput"], ordinal);
            }
            "assistant/message" | "assistant/attempt" => {
                if kind == "assistant/message" {
                    self.content(&data["message"]["content"], ordinal);
                }
                if let Some(stream) = data["stream"].as_array() {
                    for record in stream {
                        if record["type"] == "chunk" && record["chunk"]["type"] == "block-end" {
                            self.block(&record["chunk"]["block"], ordinal);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn content(&mut self, content: &Value, ordinal: u64) {
        if let Some(blocks) = content.as_array() {
            for block in blocks {
                self.block(block, ordinal);
            }
        }
    }

    fn block(&mut self, block: &Value, ordinal: u64) {
        let directory = match block["type"].as_str() {
            Some("image") => "objects",
            Some("file") => "file-objects",
            Some("tool-result") if self.version.is_some_and(|version| version < 4) => {
                self.content(&block["content"], ordinal);
                return;
            }
            _ => return,
        };
        let digest = block
            .pointer("/attachment/attachmentId")
            .and_then(Value::as_str)
            .and_then(|id| id.strip_prefix("sha256:"));
        let Some(digest) = digest.filter(|digest| {
            digest.len() == 64
                && digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) else {
            self.issue("deepseek-attachment-reference-invalid", Some(ordinal));
            return;
        };
        self.reference(
            "attachment",
            format!("attachments/v1/{directory}/{}/{digest}", &digest[..2]),
            ordinal,
        );
    }
}

fn message<'a>(payload: &'a Value, kind: &str) -> Option<&'a Value> {
    match kind {
        "user/message" => payload.get("data"),
        "system/message" | "developer/message" | "assistant/message" | "tool/result" => {
            payload.pointer("/data/message")
        }
        _ => None,
    }
}

fn role_for_event(kind: &str) -> Option<&'static str> {
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
fn valid_record(payload: &Value, kind: &str) -> bool {
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

fn native_text(payload: &Value, kind: &str, message: Option<&Value>) -> String {
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
fn content_has_spill_notice(content: &Value) -> bool {
    content.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| match block["type"].as_str() {
            Some("text") => block["text"].as_str().is_some_and(has_spill_notice),
            Some("tool-result") => content_has_spill_notice(&block["content"]),
            _ => false,
        })
    })
}

fn has_spill_notice(text: &str) -> bool {
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
const KNOWN_EVENTS: &[&str] = &[
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
