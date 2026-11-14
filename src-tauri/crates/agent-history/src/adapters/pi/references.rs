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
            self.reference_limit(ordinal);
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
            self.reference_limit(ordinal);
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

    pub(super) fn reference_limit(&mut self, ordinal: u64) {
        if !self.reference_limit_reported {
            self.issue("pi-reference-limit", Some(ordinal));
            self.reference_limit_reported = true;
        }
        self.complete = false;
    }
}
