//! Responsibilities: preserve Continue envelopes and classic Cline/Roo message arrays.
//! Not responsible for: SDK stores, native restore, attachment reads or native replay.
//! Evidence and version boundaries: research/agent-history/ecosystem.md.

use std::io::BufRead;

use serde_json::Value;

use crate::{
    Artifact, Diagnostic, Harness, NativeEvent, ParseContext, ParseReport, Result, SessionMetadata,
    SourceRoot,
};

use super::{Adapter, common};

mod discovery;
mod document;

use document::Document;

/// Reads Continue's persisted session envelopes independently of its staleable index.
pub struct ContinueAdapter;

/// Reads the classic task JSON format still supported by Cline's legacy-state reader.
pub struct ClineAdapter;

/// Preserves Roo's API messages and context-replacement facts without rewriting them.
pub struct RooCodeAdapter;

impl Adapter for ContinueAdapter {
    fn harness(&self) -> Harness {
        Harness::Continue
    }

    fn retention(&self, artifact: &Artifact) -> &'static str {
        if artifact.kind == crate::ArtifactKind::Transcript {
            "document"
        } else {
            super::default_retention(artifact)
        }
    }

    fn needs_first_line_end(&self) -> bool {
        false
    }

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        source_id: &str,
        _path: &str,
        _first_digest: &str,
        _first: &Value,
    ) -> super::identity::ConversationKey {
        // A rewritten document has no content prefix that survives a normal save.
        super::identity::ConversationKey::new(session.native_id.clone(), source_id.into())
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        discovery::discover_continue(source, emit, issues)
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> Result<()>,
    ) -> Result<ParseReport> {
        let mut document = Document::new(input, context)?;
        let mut session = SessionMetadata::default();
        let mut seen = 0_u8;
        let mut event_issues = Vec::new();
        document.object(&mut |key, document| {
            let bit = match key {
                "sessionId" => 1,
                "title" => 2,
                "workspaceDirectory" => 4,
                "chatModelTitle" => 8,
                "history" => 16,
                _ => 0,
            };
            if bit != 0 && seen & bit != 0 {
                document.issue("duplicate_metadata_field", None);
                document.value(None)?;
                return Ok(());
            }
            seen |= bit;
            if key == "history" {
                return document.array(&mut |payload, ordinal| {
                    let (event, recognized) = message_event(payload, ordinal, true);
                    if !recognized {
                        issue(
                            &mut event_issues,
                            context,
                            Some(ordinal),
                            "unrecognized_history_item",
                        );
                    }
                    emit(event)
                });
            }
            let value = document.value(None)?;
            if bit != 0 && value.as_ref().is_some_and(|value| !value.is_string()) {
                document.issue("unexpected_metadata_type", None);
            }
            let text = value.as_ref().and_then(nonempty_string);
            match key {
                "sessionId" => session.native_id = text.unwrap_or_default(),
                "title" => session.title = text,
                "workspaceDirectory" => session.project_path = text,
                "chatModelTitle" => session.model = text,
                _ => {}
            }
            Ok(())
        })?;
        if seen & 16 == 0 {
            document.issue("missing_history_array", None);
        }
        if session.native_id.is_empty() {
            document.issue("missing_native_session_id", None);
        }
        let mut report = document.finish()?;
        merge_issues(&mut report, event_issues);
        if !session.native_id.is_empty() {
            report.session = Some(session);
        }
        Ok(report)
    }
}

impl Adapter for ClineAdapter {
    fn harness(&self) -> Harness {
        Harness::Cline
    }

    fn retention(&self, artifact: &Artifact) -> &'static str {
        if artifact.kind == crate::ArtifactKind::Transcript {
            "document"
        } else {
            super::default_retention(artifact)
        }
    }

    fn needs_first_line_end(&self) -> bool {
        false
    }

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        source_id: &str,
        _path: &str,
        _first_digest: &str,
        _first: &Value,
    ) -> super::identity::ConversationKey {
        // A rewritten document has no content prefix that survives a normal save.
        super::identity::ConversationKey::new(session.native_id.clone(), source_id.into())
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        discovery::discover_tasks(source, emit, issues, false)
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> Result<()>,
    ) -> Result<ParseReport> {
        parse_task(input, context, emit)
    }
}

impl Adapter for RooCodeAdapter {
    fn harness(&self) -> Harness {
        Harness::RooCode
    }

    fn retention(&self, artifact: &Artifact) -> &'static str {
        if artifact.kind == crate::ArtifactKind::Transcript {
            "document"
        } else {
            super::default_retention(artifact)
        }
    }

    fn needs_first_line_end(&self) -> bool {
        false
    }

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        source_id: &str,
        _path: &str,
        _first_digest: &str,
        _first: &Value,
    ) -> super::identity::ConversationKey {
        // A rewritten document has no content prefix that survives a normal save.
        super::identity::ConversationKey::new(session.native_id.clone(), source_id.into())
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        discovery::discover_tasks(source, emit, issues, true)
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> Result<()>,
    ) -> Result<ParseReport> {
        parse_task(input, context, emit)
    }
}

fn parse_task(
    input: &mut dyn BufRead,
    context: &ParseContext,
    emit: &mut dyn FnMut(NativeEvent) -> Result<()>,
) -> Result<ParseReport> {
    let mut document = Document::new(input, context)?;
    let mut event_issues = Vec::new();
    let mut first_time: Option<i64> = None;
    let mut last_time: Option<i64> = None;
    document.array(&mut |payload, ordinal| {
        let (event, recognized) = message_event(payload, ordinal, false);
        if !recognized {
            issue(&mut event_issues, context, Some(ordinal), "unrecognized_api_message");
        }
        if let Some(timestamp) = event.timestamp {
            first_time = Some(first_time.map_or(timestamp, |previous| previous.min(timestamp)));
            last_time = Some(last_time.map_or(timestamp, |previous| previous.max(timestamp)));
        }
        emit(event)
    })?;
    // The immediate native directory is the task ID, not an encoded project path.
    // A bare imported API array has no reliable session identity.
    let task_id = context
        .relative_path
        .rsplit('/')
        .nth(1)
        .filter(|name| !name.is_empty() && !matches!(*name, "tasks" | "." | ".."));
    if task_id.is_none() {
        document.issue("missing_native_task_id", None);
    }
    let mut report = document.finish()?;
    merge_issues(&mut report, event_issues);
    report.session = task_id.map(|native_id| SessionMetadata {
        native_id: native_id.into(),
        project_path: context.project_hint.clone(),
        // These are the observed native message-time bounds; no date is inferred
        // from the numeric task-directory name or filesystem metadata.
        created_at: first_time,
        updated_at: last_time,
        ..Default::default()
    });
    Ok(report)
}

fn message_event(payload: Value, ordinal: u64, wrapped: bool) -> (NativeEvent, bool) {
    let message = if wrapped { payload.get("message").unwrap_or(&Value::Null) } else { &payload };
    let role = message.get("role").and_then(nonempty_string);
    let native_kind = message.get("type").and_then(Value::as_str);
    let recognized = message.is_object() && (role.is_some() || native_kind == Some("reasoning"));
    let mut text = common::text(message.get("content").unwrap_or(&Value::Null));
    if let Some(reasoning) = message.get("reasoning_content").and_then(Value::as_str) {
        append_text(&mut text, reasoning);
    }
    if native_kind == Some("reasoning") {
        if let Some(reasoning) = message.get("text").and_then(Value::as_str) {
            append_text(&mut text, reasoning);
        }
        append_text(&mut text, &common::text(message.get("summary").unwrap_or(&Value::Null)));
    }
    let native_id = message.get("id").and_then(nonempty_string);
    let timestamp = if wrapped { None } else { message.get("ts").and_then(common::timestamp) };
    let event = NativeEvent {
        references: Vec::new(),
        ordinal,
        native_id,
        parent_id: None,
        kind: native_kind.unwrap_or(if recognized { "message" } else { "unknown" }).into(),
        role,
        timestamp,
        text,
        payload,
    };
    (event, recognized)
}

fn append_text(output: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(text);
}

fn nonempty_string(value: &Value) -> Option<String> {
    value.as_str().filter(|text| !text.is_empty()).map(str::to_owned)
}

fn issue(issues: &mut Vec<Diagnostic>, context: &ParseContext, ordinal: Option<u64>, code: &str) {
    common::push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: Some(context.relative_path.clone()),
            ordinal,
            detail: None,
        },
    );
}

fn merge_issues(report: &mut ParseReport, issues: Vec<Diagnostic>) {
    if !issues.is_empty() {
        report.complete = false;
    }
    for diagnostic in issues {
        common::push_diagnostic(&mut report.diagnostics, diagnostic);
    }
}
