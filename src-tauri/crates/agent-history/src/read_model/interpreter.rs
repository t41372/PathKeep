//! Replays public native fields using only one object and its header context.
//! Session metadata, coverage diagnostics and reference resolution stay with the builder.
use super::{InterpretedRecord, PhysicalOrdinal, parts};
use crate::{
    Error, Harness, Result,
    adapters::{claude, codex, deepseek, pi, session_json},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Version one of the context needed to reconstruct a public native event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RecordContext {
    Claude,
    Codex,
    Pi { version: Option<u64>, previous_legacy_id: Option<String> },
    Deepseek { version: Option<u64> },
    Continue,
    Cline,
    RooCode,
}

impl RecordContext {
    pub(super) fn new(harness: Harness) -> Self {
        match harness {
            Harness::ClaudeCode => Self::Claude,
            Harness::Codex => Self::Codex,
            Harness::Pi => Self::Pi { version: None, previous_legacy_id: None },
            Harness::Deepseek => Self::Deepseek { version: None },
            Harness::Continue => Self::Continue,
            Harness::Cline => Self::Cline,
            Harness::RooCode => Self::RooCode,
        }
    }

    pub(super) fn header(&mut self, value: &Value, ordinal: PhysicalOrdinal, first_object: bool) {
        if value["type"] != "session" {
            return;
        }
        match self {
            Self::Pi { version, .. } if first_object => *version = pi::header_version(value),
            Self::Deepseek { version } if ordinal.0 == 0 => {
                *version = deepseek::header_fields(value).map(|(_, v, _)| v)
            }
            _ => {}
        }
    }

    pub(super) fn advance(&mut self, value: &Value, ordinal: PhysicalOrdinal) {
        if let Self::Pi { version: Some(1), previous_legacy_id } = self
            && value["type"] != "session"
        {
            *previous_legacy_id = Some(pi::legacy_id(value, ordinal.0));
        }
    }

    /// Pure interpretation for indexing, export and reader windows. Capture-only
    /// reference occurrences are attached separately by each sequential builder.
    pub fn interpret(&self, ordinal: PhysicalOrdinal, value: Value) -> Result<InterpretedRecord> {
        if !value.is_object() {
            return Err(Error::InvalidInput("expected a native object".into()));
        }
        let n = ordinal.0;
        let event = match self {
            Self::Claude => claude::native_event(value, n),
            Self::Codex => codex::native_event(value, n),
            Self::Pi { version, previous_legacy_id } => {
                pi::native_event(value, n, *version, previous_legacy_id.as_deref())
            }
            Self::Deepseek { version } => deepseek::native_event(value, n, *version),
            Self::Continue => session_json::message_event(value, n, true).0,
            Self::Cline | Self::RooCode => session_json::message_event(value, n, false).0,
        };
        let boundary = match self {
            Self::Claude => claude::boundary(&event),
            Self::Codex => codex::boundary(&event),
            Self::Pi { .. } => pi::boundary(&event),
            Self::Deepseek { version } => deepseek::boundary(&event, *version),
            _ => None,
        };
        Ok(InterpretedRecord { boundary, parts: parts::render(self, &event), event })
    }
}
