//! Responsibilities: identify Codex attachment occurrences while deduplicating target work.
//! Not responsible for: opening paths or declaring missing attachments to be unread records.
use super::{ParseState, common};
use crate::{Diagnostic, Reference};
use serde_json::Value;

impl ParseState<'_> {
    pub(super) fn media(&mut self, value: &Value, ordinal: u64) {
        let Some(parts) = value.as_array() else { return };
        for part in parts {
            match part["type"].as_str() {
                Some("local_image" | "local_audio") => self.local_reference(part, "path"),
                Some("input_image" | "image" | "input_audio" | "audio") => {
                    self.remote_media(part, ordinal);
                }
                _ => {}
            }
        }
    }

    pub(super) fn remote_media(&mut self, part: &Value, ordinal: u64) {
        if let Some(id) = part["file_id"].as_str() {
            self.reference("remote", id, None, None);
            self.remote_notice(ordinal);
        }
        for key in ["image_url", "audio_url"] {
            if let Some(url) = part[key].as_str() {
                self.remote_url(url, ordinal);
            }
        }
    }

    pub(super) fn remote_url(&mut self, url: &str, ordinal: u64) {
        if !url.starts_with("data:") {
            self.reference("remote", url, None, None);
            self.remote_notice(ordinal);
        }
    }

    pub(super) fn remote_notice(&mut self, ordinal: u64) {
        common::push_diagnostic(
            &mut self.issues,
            Diagnostic {
                code: "codex-remote-attachment-unavailable".into(),
                path: None,
                ordinal: Some(ordinal),
                detail: Some("remote".into()),
            },
        );
    }

    pub(super) fn local_reference(&mut self, value: &Value, key: &str) {
        if let Some(path) = value[key].as_str() {
            self.reference("attachment", path, None, None);
        }
    }

    pub(super) fn reference(
        &mut self,
        kind: &str,
        target: &str,
        end_byte_offset: Option<u64>,
        end_ordinal_exclusive: Option<u64>,
    ) {
        if target.len() > common::REFERENCE_TARGET_BYTES {
            self.issue("codex-reference-budget", None);
            return;
        }
        if kind != "lineage" {
            self.event_references.push(Reference {
                kind: kind.into(),
                target: target.into(),
                end_byte_offset,
                end_ordinal_exclusive,
            });
        }
        let identity = (kind.to_owned(), target.to_owned());
        if self.seen_references.contains(&identity) {
            return;
        }
        if common::push_reference(
            &mut self.references,
            Reference {
                kind: kind.into(),
                target: target.into(),
                end_byte_offset,
                end_ordinal_exclusive,
            },
            &mut self.issues,
        ) {
            self.seen_references.insert(identity);
        } else {
            self.complete = false;
        }
    }
}
