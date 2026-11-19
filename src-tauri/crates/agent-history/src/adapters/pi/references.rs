//! Responsibilities: retain Pi reference occurrences without repeated target capture work.
//! Not responsible for: reading paths or changing SessionManager payloads.
use super::{ParseState, common};
use crate::Reference;

impl ParseState<'_> {
    pub(super) fn reference(&mut self, kind: &str, target: Option<&str>, ordinal: u64) {
        let Some(target) = target.filter(|target| !target.is_empty()) else {
            return;
        };
        if target.len() > common::REFERENCE_TARGET_BYTES {
            self.reference_limit(kind, target, ordinal);
            return;
        }
        self.event_references.push(Reference {
            kind: kind.into(),
            target: target.into(),
            end_byte_offset: None,
            end_ordinal_exclusive: None,
        });
        let key = (kind.to_owned(), target.to_owned());
        if self.reference_keys.contains(&key) {
            return;
        }
        if self.reference_bytes.saturating_add(target.len()) > common::REFERENCE_TOTAL_BYTES
            || self.references.len() >= common::REFERENCE_LIMIT
        {
            self.reference_limit(kind, target, ordinal);
            return;
        }
        self.reference_keys.insert(key);
        self.reference_bytes += target.len();
        self.references.push(Reference {
            kind: kind.into(),
            target: target.into(),
            end_byte_offset: None,
            end_ordinal_exclusive: None,
        });
    }

    pub(super) fn reference_limit(&mut self, kind: &str, target: &str, ordinal: u64) {
        let local_output =
            kind == "tool-output" && crate::capture::references::local_path(target).is_ok();
        if local_output {
            if !self.local_output_limit_reported {
                self.issue("pi-local-output-limit", Some(ordinal));
                self.local_output_limit_reported = true;
            }
        } else if !self.reference_limit_reported {
            self.issue("pi-reference-limit", Some(ordinal));
            self.reference_limit_reported = true;
        }
        self.complete = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropped_local_output_has_a_save_warning_even_after_a_remote_limit_note() {
        let context = crate::ParseContext::default();
        let mut state = ParseState::new(&context);
        for n in 0..4096 {
            state.reference("tool-output", Some(&format!("https://example.invalid/{n}")), n);
        }
        state.reference("tool-output", Some("https://example.invalid/overflow"), 4096);
        assert!(!state.diagnostics.iter().any(crate::Diagnostic::save_warning));
        state.reference("tool-output", Some("outputs/saved-locally.txt"), 4097);
        assert!(state.diagnostics.iter().any(crate::Diagnostic::save_warning));
    }
}
