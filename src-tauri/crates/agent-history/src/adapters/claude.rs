//! Responsibilities: retain Claude transcript events and expose conservative session facts.
//! Not responsible for choosing a visible branch, summing usage, or repairing Claude storage.

mod discovery;

use std::{collections::HashSet, io::BufRead};

use serde_json::Value;

use super::{Adapter, common};
use crate::{Result, model::*};

/// Keeps Claude Code and local Cowork evidence readable without running either product.
pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn publishes_transcript(&self, relative_path: &str) -> bool {
        let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
        !(name.contains(".jsonl.superseded-")
            || (name.contains(".orphaned-") && name.ends_with(".jsonl")))
    }

    fn harness(&self) -> Harness {
        Harness::ClaudeCode
    }

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        _source_id: &str,
        path: &str,
        first_digest: &str,
        first: &Value,
    ) -> super::identity::ConversationKey {
        // Claude copies a parent's sessionId into subagents and fork prefixes.
        // The owned filename/layout names this conversation, never that record field.
        let mut key =
            super::identity::ConversationKey::new(session.native_id.clone(), first_digest.into());
        if let Some((folder, below)) = path.split_once("/subagents/") {
            let parent = folder.rsplit('/').next().unwrap_or(folder);
            key.native_id = format!("{parent}/{below}");
            key.relation.parent_path = Some(format!("{folder}.jsonl"));
            key.relation.parent_native_id = Some(parent.into());
        } else if let Some(name) =
            path.rsplit('/').next().and_then(|name| name.split_once(".jsonl").map(|(stem, _)| stem))
        {
            key.native_id = name.into();
            if let Some(id) = first["sessionId"].as_str().filter(|id| *id != name) {
                key.relation.forked_from_native_id = Some(id.into());
                if let Some((folder, _)) = path.rsplit_once('/') {
                    key.relation.fork_path = Some(format!("{folder}/{id}.jsonl"));
                }
            }
        }
        key
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        self.discover_cancellable(&crate::capture::Discovery::default(), source, emit, issues)
    }

    fn discover_cancellable(
        &self,
        cancel: &crate::capture::Discovery,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        discovery::discover(cancel, source, emit, issues)
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> Result<()>,
    ) -> Result<ParseReport> {
        let mut facts = SessionFacts::new(context);
        let mut diagnostics = Vec::new();
        let mut semantic_issues = Vec::new();
        let mut references = Vec::new();
        let mut reference_keys = HashSet::new();
        let mut references_complete = true;
        let stats = common::records(
            input,
            context,
            &mut |payload, ordinal| {
                facts.observe(&payload);
                let mut event_references = Vec::new();
                if let Some(attachment) = payload.get("attachment") {
                    let reference = match string(attachment, "type") {
                        Some("file" | "compact_file_reference") => {
                            string(attachment, "filename").map(|path| ("attachment", path))
                        }
                        Some("task_status") => {
                            string(attachment, "outputFilePath").map(|path| ("tool-output", path))
                        }
                        _ => None,
                    };
                    if let Some((kind, target)) = reference {
                        let reference = Reference {
                            kind: kind.into(),
                            target: target.into(),
                            end_byte_offset: None,
                            end_ordinal_exclusive: None,
                        };
                        event_references.push(reference.clone());
                        let key = (reference.kind.clone(), reference.target.clone());
                        if !reference_keys.contains(&key) {
                            let accepted = common::push_reference(
                                &mut references,
                                reference,
                                &mut semantic_issues,
                            );
                            if accepted {
                                reference_keys.insert(key);
                            }
                            references_complete &= accepted;
                        }
                    }
                }
                let kind = string(&payload, "type").unwrap_or("unknown");
                let message = payload.get("message");
                let timestamp = event_timestamp(&payload);
                let event = NativeEvent {
                    references: event_references,
                    ordinal,
                    native_id: string(&payload, "uuid").map(str::to_owned),
                    parent_id: string(&payload, "parentUuid").map(str::to_owned),
                    kind: kind.to_owned(),
                    role: message
                        .and_then(|message| string(message, "role"))
                        .or(match kind {
                            "user" => Some("user"),
                            "assistant" => Some("assistant"),
                            _ => None,
                        })
                        .map(str::to_owned),
                    timestamp,
                    text: common::text(
                        message
                            .and_then(|value| value.get("content"))
                            .or_else(|| payload.get("content"))
                            .unwrap_or(&Value::Null),
                    ),
                    payload,
                };
                emit(event)
            },
            &mut diagnostics,
        )?;
        for issue in semantic_issues {
            common::push_diagnostic(&mut diagnostics, issue);
        }
        if facts.mixed_origins {
            common::push_diagnostic(
                &mut diagnostics,
                Diagnostic {
                    code: "claude-mixed-origins".into(),
                    path: Some(context.relative_path.clone()),
                    ordinal: None,
                    detail: None,
                },
            );
        }
        if facts.mixed_session_ids {
            common::push_diagnostic(
                &mut diagnostics,
                Diagnostic {
                    code: "claude-multiple-session-ids".into(),
                    path: Some(context.relative_path.clone()),
                    ordinal: None,
                    detail: None,
                },
            );
        }
        let session = (stats.emitted_records > 0).then(|| facts.finish(context));
        if let Some(parent) =
            session.as_ref().and_then(|session| session.parent_session_id.as_ref())
        {
            references_complete &= common::push_reference(
                &mut references,
                Reference {
                    kind: "lineage".into(),
                    target: parent.clone(),
                    end_byte_offset: None,
                    end_ordinal_exclusive: None,
                },
                &mut diagnostics,
            );
        }
        Ok(ParseReport {
            session,
            event_count: stats.emitted_records,
            diagnostics,
            references,
            complete: stats.complete && references_complete,
        })
    }
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|value| !value.is_empty())
}

fn event_timestamp(value: &Value) -> Option<i64> {
    value
        .get("timestamp")
        .and_then(common::timestamp)
        .or_else(|| value.get("_audit_timestamp").and_then(common::timestamp))
}

/// Collects bounded scalar metadata; event payloads retain all conflicting evidence.
struct SessionFacts {
    native_id: Option<String>,
    project_path: Option<String>,
    title: Option<String>,
    title_priority: u8,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    origin: Option<&'static str>,
    mixed_origins: bool,
    mixed_session_ids: bool,
    version: Option<String>,
    model: Option<String>,
}

impl SessionFacts {
    fn new(context: &ParseContext) -> Self {
        Self {
            native_id: None,
            project_path: context.project_hint.clone(),
            title: None,
            title_priority: 0,
            created_at: None,
            updated_at: None,
            origin: None,
            mixed_origins: false,
            mixed_session_ids: false,
            version: None,
            model: None,
        }
    }

    fn observe(&mut self, value: &Value) {
        if let Some(id) = string(value, "sessionId").or_else(|| string(value, "session_id")) {
            match &self.native_id {
                Some(previous) if previous != id => self.mixed_session_ids = true,
                None => self.native_id = Some(id.to_owned()),
                _ => {}
            }
        }
        if let Some(cwd) = string(value, "cwd") {
            self.project_path = Some(cwd.to_owned());
        }
        if let Some(version) =
            string(value, "version").or_else(|| string(value, "claude_code_version"))
        {
            self.version = Some(version.to_owned());
        }
        if let Some(model) = value
            .get("message")
            .and_then(|message| string(message, "model"))
            .or_else(|| string(value, "model"))
        {
            self.model = Some(model.to_owned());
        }
        if let Some(timestamp) = event_timestamp(value) {
            self.created_at = Some(self.created_at.map_or(timestamp, |old| old.min(timestamp)));
            self.updated_at = Some(self.updated_at.map_or(timestamp, |old| old.max(timestamp)));
        }
        if let Some(entrypoint) = string(value, "entrypoint") {
            let origin = match entrypoint {
                "cli" => "cli",
                "claude-desktop" => "desktop",
                "local-agent" => "cowork",
                _ => "unknown",
            };
            if self.origin.is_some_and(|previous| previous != origin) {
                self.mixed_origins = true;
            } else if self.origin.is_none() {
                self.origin = Some(origin);
            }
        }
        let title = match string(value, "type") {
            Some("custom-title") => string(value, "customTitle").map(|title| (3, title)),
            Some("ai-title") => string(value, "aiTitle").map(|title| (2, title)),
            Some("summary") => string(value, "summary").map(|title| (1, title)),
            _ => None,
        };
        if let Some((priority, title)) =
            title.filter(|(priority, _)| *priority >= self.title_priority)
        {
            self.title_priority = priority;
            self.title = Some(title.to_owned());
        }
    }

    fn finish(self, context: &ParseContext) -> SessionMetadata {
        let parts: Vec<_> = context.relative_path.split('/').collect();
        let parent_session_id = parts.iter().enumerate().find_map(|(index, part)| {
            (*part == "subagents"
                && index >= 2
                && parts[index - 1] != "projects"
                && index + 1 < parts.len())
            .then(|| parts[index - 1].to_owned())
        });
        let cowork_body = parts.iter().any(|part| part.starts_with("local_"));
        let inferred_origin = if cowork_body
            && (parts.last() == Some(&"audit.jsonl")
                || parts.windows(2).any(|parts| parts == [".claude", "projects"]))
        {
            "cowork"
        } else {
            "unknown"
        };
        let filename = parts.last().copied().unwrap_or("unknown");
        let fallback_id = filename.split(".jsonl").next().unwrap_or(filename);
        SessionMetadata {
            native_id: self.native_id.unwrap_or_else(|| fallback_id.to_owned()),
            project_path: self.project_path,
            title: self.title,
            created_at: self.created_at,
            updated_at: self.updated_at,
            origin: if self.mixed_origins {
                "unknown"
            } else {
                self.origin.unwrap_or(inferred_origin)
            }
            .into(),
            harness_version: self.version,
            parent_session_id,
            forked_from_session_id: None,
            model: self.model,
        }
    }
}
