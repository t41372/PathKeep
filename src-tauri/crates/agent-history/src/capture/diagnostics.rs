//! Separate evidence that could not be saved from limitations of an interpretation.
//! Native diagnostic samples remain available even when a run succeeds.
use crate::{Archive, Diagnostic, Harness, Result};
use serde::Serialize;

impl Diagnostic {
    pub(crate) fn save_warning(&self) -> bool {
        match self.code.as_str() {
            // The selected scope or part of its enumeration was inaccessible.
            "root-missing"
            | "root-unreadable"
            | "root-changed"
            | "root_unreadable"
            | "root_changed"
            | "destination-unavailable"
            | "source_discovery_failed"
            | "source_capture_unavailable"
            | "source_unavailable"
            | "source_not_directory"
            | "source-directory-unreadable"
            | "source-entry-unreadable"
            | "codex-source-enumeration-failed"
            | "codex-source-not-directory"
            | "codex-source-unavailable"
            | "deepseek-session-root-unreadable"
            | "deepseek-attachment-root-unreadable"
            | "deepseek-directory-unreadable"
            | "deepseek-directory-entry-unreadable"
            | "deepseek-session-root-invalid"
            | "deepseek-attachment-root-invalid"
            | "deepseek-directory-not-regular"
            | "source_depth_limit"
            | "deepseek-directory-depth-exceeded"
            | "non_utf8_path"
            | "source_path_unrepresentable"
            | "codex-source-path-unrepresentable"
            | "claude-non-utf8-path"
            | "source-path-outside-root"
            | "claude-source-layout-unrecognized"
            | "task_layout_unrecognized"
            | "task_transcript_unavailable"
            | "git_unreadable"
            | "git_file_too_large"
            | "sqlite_hidden_rowid_unavailable"
            | "lineage_missing"
            | "lineage_authority_unavailable"
            | "pi-local-output-limit"
            | "capture_warnings_omitted" => true,
            "attachment_not_retained" => {
                matches!(self.detail.as_deref(), Some("missing" | "authority_unavailable"))
            }
            _ => false,
        }
    }

    pub(crate) fn omission(&self) -> bool {
        matches!(self.code.as_str(), "diagnostics_omitted" | "capture_warnings_omitted")
    }

    /// Only a parser that recognized a numeric, newer *format* sets this detail.
    pub(crate) fn future_version(&self) -> Option<&str> {
        matches!(self.code.as_str(), "pi-unsupported-version" | "deepseek-format-limited")
            .then_some(self.detail.as_deref())
            .flatten()
            .filter(|v| v.parse::<u64>().is_ok())
    }
}

/// A sampled save failure, with the selected source when capture reached it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWarning {
    #[serde(flatten)]
    pub diagnostic: Diagnostic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<Harness>,
}

/// Run-level counts separate failures to retain evidence from notes about its use.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOutcome {
    pub warning_count: u64,
    pub note_count: u64,
    pub new_note_count: u64,
    pub warnings: Vec<SaveWarning>,
}

impl CaptureOutcome {
    pub(super) fn warning(&mut self, issue: &Diagnostic, scope: Option<(&str, Harness)>) {
        let count = if issue.omission() {
            issue.detail.as_deref().and_then(|n| n.parse().ok()).unwrap_or(1)
        } else {
            1
        };
        self.warning_count = self.warning_count.saturating_add(count);
        if self.warnings.len() < crate::adapters::common::DIAGNOSTIC_SAMPLES {
            self.warnings.push(SaveWarning {
                diagnostic: issue.clone(),
                root_id: scope.map(|(id, _)| id.to_owned()),
                tool: scope.map(|(_, tool)| tool),
            });
        }
    }

    pub(super) fn observe(
        &mut self,
        archive: &Archive,
        run: i64,
        root: &str,
        tool: Harness,
        file: Option<&str>,
        issues: &[Diagnostic],
    ) -> Result<()> {
        let mut notes = Vec::new();
        for issue in issues {
            if issue.save_warning() {
                self.warning(issue, Some((root, tool)));
            } else if !issue.omission() {
                notes.push(issue.clone());
            }
        }
        if !notes.is_empty() {
            let (seen, fresh) = archive.record_capture_notes(run, root, tool, file, notes)?;
            self.note_count += seen;
            self.new_note_count += fresh;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct RootOutcome {
    changed_root: bool,
    unreadable: bool,
    unknown: bool,
    changing: bool,
}
impl RootOutcome {
    pub fn include(&mut self, issues: &[Diagnostic]) {
        for issue in issues {
            self.changed_root |= issue.code == "root_changed";
            self.unreadable |= issue.save_warning();
            self.unknown |= issue.code.contains("unsupported")
                || matches!(
                    issue.code.as_str(),
                    "deepseek-format-limited"
                        | "session_metadata_unavailable"
                        | "source_decompression_failed"
                );
            self.changing |= matches!(
                issue.code.as_str(),
                "source_changed_during_capture" | "first_record_in_progress" | "incomplete_tail"
            );
        }
    }
    pub fn state(&self) -> &'static str {
        if self.changed_root {
            "root-changed"
        } else if self.unreadable {
            "unreadable"
        } else if self.unknown {
            "unknown-format"
        } else if self.changing {
            "changing"
        } else {
            "available"
        }
    }
}
