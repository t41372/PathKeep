//! Responsibilities: carry adapter-chosen keys and native cross-conversation links.
//! Not responsible for: looking up another conversation or authorizing reads of its files.
use crate::Harness;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Native link evidence remains meaningful before the referenced source is captured.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceRelation {
    pub base_native_id: Option<String>,
    pub base_end_ordinal: Option<u64>,
    pub base_end_byte: Option<u64>,
    pub forked_from_native_id: Option<String>,
    pub parent_path: Option<String>,
    pub parent_native_id: Option<String>,
    pub fork_path: Option<String>,
}

/// No capture clock, source-root path, issued counter or lookup contributes to this key.
#[derive(Clone, Debug)]
pub struct ConversationKey {
    pub native_id: String,
    pub origin_mark: String,
    pub relation: SourceRelation,
    pub generation: u64,
    pub first_record_digest: String,
}

impl ConversationKey {
    /// Length boundaries prevent ambiguous concatenations of native identifiers.
    pub fn id(&self, harness: Harness, source_id: &str) -> String {
        let native = if self.native_id.is_empty() { source_id } else { &self.native_id };
        let mut hash = Sha256::new();
        hash.update((b"pathkeep-agent-conversation-v1".len() as u64).to_be_bytes());
        hash.update(b"pathkeep-agent-conversation-v1");
        for part in [harness.as_str(), native, &self.origin_mark] {
            hash.update((part.len() as u64).to_be_bytes());
            hash.update(part.as_bytes());
        }
        hex::encode(hash.finalize())
    }
}

/// Generic key assembly leaves every tool-specific identity decision in its adapter.
impl ConversationKey {
    pub fn new(native_id: String, origin_mark: String) -> Self {
        let origin_mark = if native_id.is_empty() { String::new() } else { origin_mark };
        Self {
            native_id,
            origin_mark,
            relation: SourceRelation::default(),
            generation: 0,
            first_record_digest: String::new(),
        }
    }
}
