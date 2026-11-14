//! Pi's native JSONL history, without invoking its mutating SessionManager.
//!
//! Responsibilities: discover session artifacts, preserve every parsed object,
//! and expose message, branch, and declared artifact evidence for later queries.
//! Not responsible for replaying model context, reading referenced files, native
//! migration, or guessing whether a session came from TUI, RPC, or a desktop app.

use std::collections::HashSet;
use std::io::BufRead;
use std::path::Path;

use serde_json::Value;

use super::{Adapter, common};
mod references;
use crate::Result;
use crate::model::{
    Artifact, ArtifactKind, Diagnostic, Harness, NativeEvent, ParseContext, ParseReport, Reference,
    SessionMetadata, SourceRoot,
};

const CURRENT_VERSION: u64 = 3;

/// Keeps Pi's original branches and extension data available independently of Pi.
pub struct PiAdapter;

impl Adapter for PiAdapter {
    fn harness(&self) -> Harness {
        Harness::Pi
    }

    fn lineage_is_path(&self) -> bool {
        // SessionManager records parentSession as a path; forks already copy their own history.
        true
    }

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        _source_id: &str,
        _path: &str,
        first_digest: &str,
        first: &Value,
    ) -> super::identity::ConversationKey {
        // SessionManager writes the header once; appends and root copies keep it.
        let native = if session.native_id.is_empty() && first["type"] == "session" {
            first["id"].as_str().unwrap_or_default().to_owned()
        } else {
            session.native_id.clone()
        };
        super::identity::ConversationKey::new(native, first_digest.into())
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> Result<()> {
        // Selecting a nested default must not hide an intermediate symlink from
        // walk_files, which only sees the root it is given and its descendants.
        for candidate in [&source.root, &source.root.join("agent")] {
            if candidate.symlink_metadata().is_ok_and(|meta| meta.file_type().is_symlink()) {
                common::push_diagnostic(
                    issues,
                    Diagnostic {
                        code: "symlink_skipped".into(),
                        path: Some(candidate.to_string_lossy().into_owned()),
                        ordinal: None,
                        detail: None,
                    },
                );
                return Ok(());
            }
        }
        let nested = source.root.join("agent/sessions");
        let direct = source.root.join("sessions");
        let root = if nested.is_dir() {
            nested
        } else if direct.is_dir() {
            direct
        } else {
            source.root.clone()
        };
        common::walk_files(
            &root,
            &mut |path| {
                if path.extension().and_then(|part| part.to_str()) != Some("jsonl") {
                    return Ok(());
                }
                let relative_path = artifact_path(&source.root, &path);
                emit(Artifact {
                    path,
                    relative_path,
                    kind: ArtifactKind::Transcript,
                    // The encoded directory is lossy. Only the header proves cwd.
                    project_hint: None,
                    generation: None,
                    current: true,
                })
            },
            issues,
        )
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> Result<()>,
    ) -> Result<ParseReport> {
        let mut state = ParseState::new(context);
        let mut read_issues = Vec::new();
        let stats = common::records(
            input,
            context,
            &mut |value, ordinal| {
                let mut event = state.event(value, ordinal);
                event.references = std::mem::take(&mut state.event_references);
                emit(event)
            },
            &mut read_issues,
        )?;
        if state.session.is_none() {
            state.issue("pi-missing-header", None);
            state.complete = false;
        }
        for issue in read_issues {
            common::push_diagnostic(&mut state.diagnostics, issue);
        }
        Ok(ParseReport {
            session: state.session,
            event_count: stats.emitted_records,
            diagnostics: state.diagnostics,
            references: state.references,
            complete: state.complete && stats.complete,
        })
    }
}

struct ParseState<'a> {
    context: &'a ParseContext,
    session: Option<SessionMetadata>,
    version: Option<u64>,
    saw_object: bool,
    previous_legacy_id: Option<String>,
    diagnostics: Vec<Diagnostic>,
    references: Vec<Reference>,
    event_references: Vec<Reference>,
    reference_keys: HashSet<(String, String)>,
    reference_bytes: usize,
    reference_limit_reported: bool,
    complete: bool,
}

impl<'a> ParseState<'a> {
    fn new(context: &'a ParseContext) -> Self {
        Self {
            context,
            session: None,
            version: None,
            saw_object: false,
            previous_legacy_id: None,
            diagnostics: Vec::new(),
            references: Vec::new(),
            event_references: Vec::new(),
            reference_keys: HashSet::new(),
            reference_bytes: 0,
            reference_limit_reported: false,
            complete: true,
        }
    }

    fn issue(&mut self, code: &str, ordinal: Option<u64>) {
        common::push_diagnostic(
            &mut self.diagnostics,
            Diagnostic {
                code: code.into(),
                path: Some(self.context.relative_path.clone()),
                ordinal,
                detail: None,
            },
        );
    }

    fn header(&mut self, value: &Value, ordinal: u64) {
        if self.saw_object || self.session.is_some() {
            self.issue("pi-unexpected-header", Some(ordinal));
            self.complete = false;
            return;
        }
        let Some(native_id) = string(value, "id") else {
            self.issue("pi-invalid-header", Some(ordinal));
            self.complete = false;
            return;
        };
        self.version = match value.get("version") {
            None => Some(1),
            Some(version) => version.as_u64(),
        };
        if !matches!(self.version, Some(1..=CURRENT_VERSION)) {
            self.issue("pi-unsupported-version", Some(ordinal));
            self.complete = false;
        } else if self.version == Some(1) {
            self.issue("pi-legacy-derived-identity", Some(ordinal));
        }
        let created_at = value.get("timestamp").and_then(common::timestamp);
        self.session = Some(SessionMetadata {
            native_id: native_id.into(),
            project_path: string(value, "cwd").map(str::to_owned),
            title: None,
            created_at,
            updated_at: created_at,
            origin: "unknown".into(),
            // The session format version is not the installed harness version.
            harness_version: None,
            // Pi's parentSession is a path, not a native session ID.
            parent_session_id: None,
            forked_from_session_id: None,
            model: None,
        });
        self.reference("lineage", string(value, "parentSession"), ordinal);
    }

    fn event(&mut self, payload: Value, ordinal: u64) -> NativeEvent {
        let kind = string(&payload, "type").unwrap_or("unknown").to_owned();
        if kind == "session" {
            self.header(&payload, ordinal);
        }
        self.saw_object = true;

        let mut native_id = string(&payload, "id").map(str::to_owned);
        let mut parent_id = string(&payload, "parentId").map(str::to_owned);
        if kind != "session" && self.version == Some(1) {
            // Ordinals are stable within a retained source revision; never mint
            // random IDs or overwrite the legacy payload to emulate a migration.
            native_id.get_or_insert_with(|| format!("legacy:{ordinal}"));
            parent_id = self.previous_legacy_id.clone();
            self.previous_legacy_id = native_id.clone();
        } else if kind != "session"
            && matches!(self.version, Some(2..=CURRENT_VERSION))
            && (native_id.is_none()
                || !payload.get("parentId").is_some_and(|p| p.is_null() || p.is_string()))
        {
            self.issue("pi-invalid-entry-identity", Some(ordinal));
            self.complete = false;
        }

        let message = if kind == "message" { payload.get("message") } else { None };
        let role = message
            .and_then(|m| string(m, "role"))
            .map(|role| if role == "hookMessage" { "custom" } else { role }.to_owned())
            .or_else(|| (kind == "custom_message").then(|| "custom".into()));
        let timestamp = payload
            .get("timestamp")
            .and_then(common::timestamp)
            .or_else(|| message.and_then(|m| m.get("timestamp")).and_then(common::timestamp));
        let text = event_text(&kind, &payload, message);
        if let Some(message) = message {
            match string(message, "role") {
                Some("bashExecution") => {
                    self.reference("tool-output", string(message, "fullOutputPath"), ordinal)
                }
                Some("toolResult") => self.reference(
                    "tool-output",
                    message.get("details").and_then(|d| string(d, "fullOutputPath")),
                    ordinal,
                ),
                None => {
                    self.issue("pi-malformed-message", Some(ordinal));
                    self.complete = false;
                }
                _ => {}
            }
        } else if kind == "message" {
            self.issue("pi-malformed-message", Some(ordinal));
            self.complete = false;
        }
        if let Some(session) = &mut self.session {
            session.updated_at = session.updated_at.into_iter().chain(timestamp).max();
            if kind == "session_info" {
                // The latest entry also owns clearing the title; falling back
                // to an older nonempty name would resurrect a removed name.
                session.title = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned);
            }
            if kind == "model_change" {
                if let Some(model) = string(&payload, "modelId") {
                    session.model = Some(model.into());
                }
            } else if role.as_deref() == Some("assistant") {
                if let Some(model) = message.and_then(|m| string(m, "model")) {
                    session.model = Some(model.into());
                }
            }
        }
        NativeEvent {
            references: Vec::new(),
            ordinal,
            native_id,
            parent_id,
            kind,
            role,
            timestamp,
            text,
            payload,
        }
    }
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().filter(|value| !value.is_empty())
}

fn event_text(kind: &str, payload: &Value, message: Option<&Value>) -> String {
    if let Some(message) = message {
        if string(message, "role") == Some("bashExecution") {
            return [string(message, "command"), string(message, "output")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n");
        }
        return message.get("content").map(content_text).unwrap_or_default();
    }
    match kind {
        "custom_message" => payload.get("content").map(common::text).unwrap_or_default(),
        "context_edit" => payload
            .get("replacement")
            .and_then(|replacement| replacement.get("content"))
            .map(content_text)
            .unwrap_or_default(),
        "compaction" | "branch_summary" => string(payload, "summary").unwrap_or_default().into(),
        "label" => string(payload, "label").unwrap_or_default().into(),
        "session_info" => string(payload, "name").unwrap_or_default().into(),
        _ => String::new(),
    }
}

fn content_text(content: &Value) -> String {
    let Some(blocks) = content.as_array() else {
        return common::text(content);
    };
    blocks
        .iter()
        .map(|block| {
            if string(block, "type") != Some("toolCall") {
                return common::text(block);
            }
            // Tool arguments belong to the recorded conversation. They are
            // searchable data, never permission to open the mentioned paths.
            let name = string(block, "name").unwrap_or_default();
            block.get("arguments").map_or_else(|| name.into(), |args| format!("{name}\n{args}"))
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn artifact_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| path.file_name().map(Path::new))
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
