//! Responsibilities: project DeepSeek native log evidence without running the harness.
//! Not responsible for: Zstandard decoding, source repair, native replay, or file reads
//! authorized by conversation text. Raw generations remain independently archived.
//!
//! Format evidence is pinned in research/agent-history/pi-deepseek.md. V4 is the
//! current logical format at upstream fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159;
//! older formats are retained with explicitly limited interpretation.

mod discovery;
mod records;
use records::*;

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

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        _source_id: &str,
        path: &str,
        _first_digest: &str,
        first: &Value,
    ) -> super::identity::ConversationKey {
        // Generation headers are rewritten; no evidence establishes a stable createdAt.
        let native = if session.native_id.is_empty() && first["type"] == "session" {
            first["id"].as_str().unwrap_or_default().to_owned()
        } else {
            session.native_id.clone()
        };
        let mut key = super::identity::ConversationKey::new(native, String::new());
        let file = path.rsplit('/').next().unwrap_or(path);
        key.generation = discovery::generation_number(file).unwrap_or(0);
        key.relation.parent_native_id = session.parent_session_id.clone();
        if first["type"] == "session" && first["origin"] == "subagent" {
            key.relation.parent_native_id = first["parentSession"].as_str().map(str::to_owned);
        }
        if key.relation.parent_native_id.is_none() {
            key.relation.forked_from_native_id = session
                .forked_from_session_id
                .clone()
                .or_else(|| first["parentSession"].as_str().map(str::to_owned));
        }
        key
    }

    fn current_source_only(&self) -> bool {
        true
    }

    fn generation_group(&self, path: &str) -> Option<String> {
        let (folder, file) = path.rsplit_once('/')?;
        discovery::generation_number(file)?;
        Some(folder.into())
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
    event_references: Vec<Reference>,
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
            event_references: Vec::new(),
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
            references: std::mem::take(&mut self.event_references),
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
            parent_session_id: if payload["origin"] == "subagent" { parent.clone() } else { None },
            forked_from_session_id: if payload["origin"] == "subagent" { None } else { parent },
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
        let reference = Reference {
            kind: kind.into(),
            target,
            end_byte_offset: None,
            end_ordinal_exclusive: None,
        };
        self.event_references.push(reference.clone());
        if self.seen_references.contains(&key) {
            return;
        }
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
