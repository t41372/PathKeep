//! Responsibilities: discover Codex-owned history and interpret native records.
//! Not responsible for decompression, SQLite snapshots, lineage materialization,
//! attachment reads, transcript deduplication, or invoking Codex.

use super::common;
mod events;
mod identity;
mod metadata;
mod references;
use super::{
    Adapter, Artifact, ArtifactKind, Diagnostic, Harness, NativeEvent, ParseContext, ParseReport,
    Reference, SessionMetadata, SourceRoot,
};
use events::{event_time, native_event};
use metadata::origin;
use serde_json::Value;
use std::collections::HashSet;

use std::io::BufRead;
use std::path::Path;

/// Keeps the durable Codex event stream independent of frontend and DB versions.
pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn harness(&self) -> Harness {
        Harness::Codex
    }

    fn conversation_key(
        &self,
        session: &SessionMetadata,
        _source_id: &str,
        _path: &str,
        _first_digest: &str,
        first: &Value,
    ) -> super::identity::ConversationKey {
        identity::key(session, first)
    }

    fn physical_source_id(&self, path: &str) -> Option<String> {
        identity::rollout_id(path).map(str::to_owned)
    }

    fn source_time(&self, path: &str) -> Option<String> {
        identity::filename_time(path).map(str::to_owned)
    }

    fn discover(
        &self,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> crate::Result<()> {
        self.discover_cancellable(&crate::capture::Discovery::default(), source, emit, issues)
    }

    fn discover_cancellable(
        &self,
        cancel: &crate::capture::Discovery,
        source: &SourceRoot,
        emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
        issues: &mut Vec<Diagnostic>,
    ) -> crate::Result<()> {
        match cancel.metadata(&source.root) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                discovery_issue(issues, "codex-source-not-directory");
                return Ok(());
            }
            Err(_) => {
                discovery_issue(issues, "codex-source-unavailable");
                return Ok(());
            }
        }
        // Validate the selected root before enumerating only known native locations.
        let entries = match cancel.read_dir(&source.root) {
            Ok(entries) => entries,
            Err(_) => {
                discovery_issue(issues, "codex-source-unavailable");
                return Ok(());
            }
        };
        for entry in entries {
            cancel.visited_entry()?;
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    discovery_issue(issues, "codex-source-enumeration-failed");
                    continue;
                }
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let kind = match name {
                "sessions" | "archived_sessions" => ArtifactKind::Transcript,
                "attachments" | "generated_images" => ArtifactKind::Attachment,
                "history.jsonl" | "session_index.jsonl" => ArtifactKind::Metadata,
                _ if is_database_name(name) => ArtifactKind::Sqlite,
                _ => continue,
            };
            let mut path_issues = Vec::new();
            common::walk_files_cancellable(
                cancel,
                &entry.path(),
                &mut |path| {
                    if kind == ArtifactKind::Transcript && !is_rollout(&path) {
                        cancel.unknown_file();
                        return Ok(());
                    }
                    let Some(relative) =
                        path.strip_prefix(&source.root).ok().and_then(Path::to_str)
                    else {
                        discovery_issue(&mut path_issues, "codex-source-path-unrepresentable");
                        return Ok(());
                    };
                    emit(Artifact {
                        relative_path: Path::new(relative)
                            .components()
                            .map(|component| component.as_os_str().to_str().unwrap_or_default())
                            .collect::<Vec<_>>()
                            .join("/"),
                        path,
                        kind,
                        project_hint: None,
                        generation: None,
                        current: true,
                    })
                },
                issues,
            )?;
            for issue in path_issues {
                common::push_diagnostic(issues, issue);
            }
        }
        Ok(())
    }

    fn parse(
        &self,
        input: &mut dyn BufRead,
        context: &ParseContext,
        emit: &mut dyn FnMut(NativeEvent) -> crate::Result<()>,
    ) -> crate::Result<ParseReport> {
        let mut state = ParseState::new(context);
        let mut line_issues = Vec::new();
        let stats = common::records(
            input,
            context,
            &mut |record, ordinal| {
                state.observe(&record, ordinal);
                let mut event = native_event(record, ordinal);
                event.references = std::mem::take(&mut state.event_references);
                emit(event)
            },
            &mut line_issues,
        )?;
        for issue in line_issues {
            common::push_diagnostic(&mut state.issues, issue);
        }
        if state.session.is_none() {
            state.issue("codex-session-metadata-missing", None);
        }
        if let Some(session) = &mut state.session {
            session.updated_at = state.latest_timestamp.or(session.created_at);
        }
        Ok(ParseReport {
            session: state.session,
            event_count: stats.emitted_records,
            diagnostics: state.issues,
            references: state.references,
            complete: stats.complete && state.complete,
        })
    }
}

pub(crate) fn is_database_name(name: &str) -> bool {
    name.strip_prefix("state_").and_then(|suffix| suffix.strip_suffix(".sqlite")).is_some_and(
        |version| !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit()),
    )
}

pub(crate) fn is_rollout(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
}

fn discovery_issue(issues: &mut Vec<Diagnostic>, code: &str) {
    common::push_diagnostic(
        issues,
        Diagnostic { code: code.into(), path: None, ordinal: None, detail: None },
    );
}

struct ParseState<'a> {
    context: &'a ParseContext,
    session: Option<SessionMetadata>,
    latest_timestamp: Option<i64>,
    issues: Vec<Diagnostic>,
    references: Vec<Reference>,
    event_references: Vec<Reference>,
    seen_references: HashSet<(String, String)>,
    complete: bool,
}

impl<'a> ParseState<'a> {
    fn new(context: &'a ParseContext) -> Self {
        Self {
            context,
            session: None,
            latest_timestamp: None,
            issues: Vec::new(),
            references: Vec::new(),
            event_references: Vec::new(),
            seen_references: HashSet::new(),
            complete: true,
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

    fn observe(&mut self, record: &Value, ordinal: u64) {
        let payload = &record["payload"];
        if let Some(timestamp) = event_time(record) {
            self.latest_timestamp =
                Some(self.latest_timestamp.map_or(timestamp, |old| old.max(timestamp)));
        }
        match record["type"].as_str() {
            Some("session_meta") if self.session.is_none() => self.metadata(payload, ordinal),
            Some("turn_context") => {
                if let (Some(session), Some(model)) = (&mut self.session, string(payload, "model"))
                {
                    session.model = Some(model);
                }
            }
            _ => {}
        }
        match record["type"].as_str() {
            Some("response_item") => {
                self.media(&payload["content"], ordinal);
                self.media(&payload["output"], ordinal);
            }
            Some("event_msg") if payload["type"] == "item_completed" => {
                let item = &payload["item"];
                self.media(&item["content"], ordinal);
                self.media(&item["output"], ordinal);
                match item["type"].as_str() {
                    Some("ImageView") => self.local_reference(item, "path"),
                    Some("ImageGeneration") => self.local_reference(item, "saved_path"),
                    _ => {}
                }
            }
            Some("event_msg") if payload["type"] == "user_message" => {
                for key in ["local_images", "local_audio"] {
                    if let Some(paths) = payload[key].as_array() {
                        for path in paths.iter().filter_map(Value::as_str) {
                            self.reference("attachment", path, None, None);
                        }
                    }
                }
                if let Some(ids) = payload["file_ids"].as_array() {
                    for id in ids.iter().filter_map(Value::as_str) {
                        self.reference("remote", id, None, None);
                        self.remote_notice(ordinal);
                    }
                }
                for key in ["images", "audio"] {
                    if let Some(urls) = payload[key].as_array() {
                        for url in urls.iter().filter_map(Value::as_str) {
                            self.remote_url(url, ordinal);
                        }
                    }
                }
                if let Some(references) = payload["audio_references"].as_array() {
                    for reference in references {
                        self.remote_media(reference, ordinal);
                    }
                }
            }
            Some("event_msg") if payload["type"] == "image_generation_end" => {
                self.local_reference(payload, "saved_path");
            }
            _ => {}
        }
    }

    fn metadata(&mut self, payload: &Value, ordinal: u64) {
        let Some(native_id) = string(payload, "id").filter(|id| !id.is_empty()) else {
            self.issue("codex-session-metadata-invalid", Some(ordinal));
            return;
        };
        let parent_session_id = string(payload, "parent_thread_id")
            .or_else(|| string(&payload["source"]["subagent"]["thread_spawn"], "parent_thread_id"));
        self.session = Some(SessionMetadata {
            native_id,
            project_path: string(payload, "cwd").or_else(|| self.context.project_hint.clone()),
            title: string(payload, "title").or_else(|| string(payload, "thread_name")),
            created_at: common::timestamp(&payload["timestamp"]),
            updated_at: None,
            origin: origin(payload).into(),
            harness_version: string(payload, "cli_version"),
            forked_from_session_id: if parent_session_id.is_none() {
                string(payload, "forked_from_id")
            } else {
                None
            },
            parent_session_id,
            model: string(payload, "model"),
        });
        if let Some(mode) = payload["history_mode"].as_str()
            && !matches!(mode, "legacy" | "paginated")
        {
            self.issue("codex-history-mode-unsupported", Some(ordinal));
        }
        if !payload["history_base"].is_null() {
            let base = &payload["history_base"];
            if let Some(target) = base["thread_id"].as_str() {
                self.reference(
                    "lineage",
                    target,
                    base["end_byte_offset"].as_u64(),
                    base["end_ordinal_exclusive"].as_u64(),
                );
            }
            // Whether the retained chain resolves is decided by the read snapshot.
        }
    }
}

fn string(value: &Value, key: &str) -> Option<String> {
    value[key].as_str().map(str::to_owned)
}
