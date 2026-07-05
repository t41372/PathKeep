//! Embedding fingerprint: the stale-detection signature stamped on every vector index.
//!
//! ## Responsibilities
//! - capture the embedding configuration that an index was built under
//!   (provider, model, effective dim, dtype, normalization, pooling, instruction, version)
//! - produce a stable, order-independent hash of that configuration
//! - answer "is an index built under fingerprint A still valid for the live config B?"
//!
//! ## Not responsible for
//! - reading the live provider config (callers build the fingerprint from a descriptor)
//! - triggering re-embedding or mutating any index (W-AI-5 wires staleness into rebuilds)
//! - persistence policy beyond defining the serializable header type
//!
//! ## Why this exists
//! D4 says PathKeep makes zero model assumptions: dim/pooling/normalization/instruction are
//! per-model and the default model can change. The fingerprint is the承載 mechanism (02 §C.4):
//! when any of those inputs change, the hash changes, the index is flagged stale, and a
//! versioned rebuild is offered. It mirrors the spirit of the Core Intelligence
//! stage-version watermark in `intelligence/incremental.rs`.

use crate::ai::traits::{EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling};
use crate::utils::sha256_hex;
use serde::{Deserialize, Serialize};

/// Current fingerprint schema version.
///
/// Bumping this invalidates every existing index even if the underlying embedding config is
/// unchanged — use it when the fingerprint's own meaning changes (e.g. a new field is added
/// to the hashed payload).
pub const EMBEDDING_FINGERPRINT_VERSION: u32 = 1;

/// The persisted header stamped on a vector index, identifying how it was embedded.
///
/// Stored in each vector/agent sidecar header (02 §A). It is `Serialize`/`Deserialize` so it
/// can ride inside a persisted index file, but the canonical identity used for comparison is
/// the [`EmbeddingFingerprint::hash`], not the struct's field-by-field equality (the hash is
/// what callers compare and store as a short token).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingFingerprint {
    /// Provider identity that produced the vectors.
    pub provider_id: String,
    /// Model identifier (a runtime string, never a hardcoded product constant).
    pub model_id: String,
    /// The actual returned vector length the index was built with.
    pub effective_dim: usize,
    /// Component dtype the provider emitted.
    pub output_dtype: EmbeddingDtype,
    /// Whether the stored vectors are L2-normalized.
    pub normalized: bool,
    /// Pooling strategy when known.
    pub pooling: EmbeddingPooling,
    /// Instruction template applied to inputs, when the model required one.
    pub instruction_template: Option<String>,
    /// Fingerprint schema version (see [`EMBEDDING_FINGERPRINT_VERSION`]).
    pub version: u32,
}

impl EmbeddingFingerprint {
    /// Builds a fingerprint directly from its parts at the current schema version.
    pub fn new(
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
        effective_dim: usize,
        output_dtype: EmbeddingDtype,
        normalized: bool,
        pooling: EmbeddingPooling,
        instruction_template: Option<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            effective_dim,
            output_dtype,
            normalized,
            pooling,
            instruction_template,
            version: EMBEDDING_FINGERPRINT_VERSION,
        }
    }

    /// Builds a fingerprint from a runtime [`EmbeddingDescriptor`] once its dim is known.
    ///
    /// Returns `None` when the descriptor has not yet observed a real vector length, because
    /// a fingerprint without an effective dim would assume a model dimension — exactly the
    /// D4 violation this module exists to prevent.
    pub fn from_descriptor(descriptor: &EmbeddingDescriptor) -> Option<Self> {
        let effective_dim = descriptor.effective_dim?;
        Some(Self::new(
            descriptor.provider_id.clone(),
            descriptor.model_id.clone(),
            effective_dim,
            descriptor.dtype,
            descriptor.normalized,
            descriptor.pooling,
            descriptor.instruction_template.clone(),
        ))
    }

    /// Returns the stable hash token for this fingerprint.
    ///
    /// The payload assembles every field on its own `name=value` line in a fixed order. The
    /// collision-resistance does NOT rely on delimiters being absent from values (the free
    /// string fields — provider/model/instruction — CAN contain `\n` or `=`); it relies on
    /// every field unconditionally emitting its own `name=` prefix, so a delimiter injected
    /// into one value can only ADD or extend lines and can never impersonate another field's
    /// prefix to cancel it out. The digest is therefore deterministic across runs/platforms and
    /// insensitive to serializer key ordering. (Tested by `injected_delimiters_do_not_collide`;
    /// if you ever change this format, keep that invariant and update the golden vector.)
    pub fn hash(&self) -> String {
        let instruction = self.instruction_template.as_deref().unwrap_or("");
        let payload = format!(
            "embedding-fingerprint/v{version}\nprovider={provider}\nmodel={model}\ndim={dim}\ndtype={dtype}\nnormalized={normalized}\npooling={pooling}\ninstruction={instruction}",
            version = self.version,
            provider = self.provider_id,
            model = self.model_id,
            dim = self.effective_dim,
            dtype = self.output_dtype.as_str(),
            normalized = self.normalized,
            pooling = self.pooling.as_str(),
            instruction = instruction,
        );
        sha256_hex(payload.as_bytes())
    }

    /// Returns `true` when two fingerprints describe an identical embedding configuration.
    ///
    /// Compares by hash so callers can persist only the short token and still answer the
    /// question without rehydrating the full struct.
    pub fn matches(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }

    /// Returns `true` when an index built under `self` is stale relative to the live `other`.
    ///
    /// Inverse of [`matches`](Self::matches); named for the call site that reads as
    /// "is the stored fingerprint stale against the current config?".
    pub fn is_stale_against(&self, other: &Self) -> bool {
        !self.matches(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EmbeddingFingerprint {
        EmbeddingFingerprint::new(
            "lmstudio-embed",
            "text-embedding-qwen3-embedding-0.6b",
            1024,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::LastToken,
            Some("query: {text}".to_string()),
        )
    }

    #[test]
    fn new_stamps_current_version() {
        assert_eq!(sample().version, EMBEDDING_FINGERPRINT_VERSION);
    }

    #[test]
    fn same_inputs_produce_same_hash() {
        assert_eq!(sample().hash(), sample().hash());
        assert!(sample().matches(&sample()));
        assert!(!sample().is_stale_against(&sample()));
    }

    #[test]
    fn changing_provider_changes_hash() {
        let mut other = sample();
        other.provider_id = "openai-embed".to_string();
        assert_ne!(sample().hash(), other.hash());
        assert!(sample().is_stale_against(&other));
    }

    #[test]
    fn changing_model_changes_hash() {
        let mut other = sample();
        other.model_id = "text-embedding-3-small".to_string();
        assert_ne!(sample().hash(), other.hash());
    }

    #[test]
    fn changing_dim_changes_hash() {
        let mut other = sample();
        other.effective_dim = 256;
        assert_ne!(sample().hash(), other.hash());
    }

    #[test]
    fn changing_dtype_changes_hash() {
        let mut other = sample();
        other.output_dtype = EmbeddingDtype::Int8;
        assert_ne!(sample().hash(), other.hash());
    }

    #[test]
    fn changing_normalized_changes_hash() {
        let mut other = sample();
        other.normalized = false;
        assert_ne!(sample().hash(), other.hash());
    }

    #[test]
    fn changing_pooling_changes_hash() {
        let mut other = sample();
        other.pooling = EmbeddingPooling::Mean;
        assert_ne!(sample().hash(), other.hash());
    }

    #[test]
    fn changing_instruction_changes_hash() {
        let mut other = sample();
        other.instruction_template = Some("document: {text}".to_string());
        assert_ne!(sample().hash(), other.hash());

        let mut none = sample();
        none.instruction_template = None;
        assert_ne!(sample().hash(), none.hash());
    }

    #[test]
    fn changing_version_changes_hash() {
        let mut other = sample();
        other.version = EMBEDDING_FINGERPRINT_VERSION + 1;
        assert_ne!(sample().hash(), other.hash());
    }

    #[test]
    fn empty_instruction_is_distinct_from_none_only_when_value_differs() {
        // None and Some("") both serialize the instruction segment as empty, so they share a
        // hash by design: an absent template and an empty template impose the same encoding.
        let mut empty = sample();
        empty.instruction_template = Some(String::new());
        let mut none = sample();
        none.instruction_template = None;
        assert_eq!(empty.hash(), none.hash());
    }

    #[test]
    fn from_descriptor_requires_known_dim_and_carries_every_field() {
        // Distinctive (non-default) values for every field so a mutant that drops/swaps any one
        // mapping in `from_descriptor` is caught (not just the three fields asserted before).
        let mut descriptor = EmbeddingDescriptor {
            provider_id: "lmstudio-embed".to_string(),
            model_id: "qwen3".to_string(),
            effective_dim: None,
            dtype: EmbeddingDtype::Int8,
            normalized: false,
            pooling: EmbeddingPooling::Mean,
            instruction_template: Some("doc: {text}".to_string()),
        };
        assert!(EmbeddingFingerprint::from_descriptor(&descriptor).is_none());

        descriptor.effective_dim = Some(512);
        let fingerprint =
            EmbeddingFingerprint::from_descriptor(&descriptor).expect("dim known fingerprint");
        assert_eq!(fingerprint.effective_dim, 512);
        assert_eq!(fingerprint.provider_id, "lmstudio-embed");
        assert_eq!(fingerprint.model_id, "qwen3");
        assert_eq!(fingerprint.output_dtype, EmbeddingDtype::Int8);
        assert!(!fingerprint.normalized);
        assert_eq!(fingerprint.pooling, EmbeddingPooling::Mean);
        assert_eq!(fingerprint.instruction_template.as_deref(), Some("doc: {text}"));
        assert_eq!(fingerprint.version, EMBEDDING_FINGERPRINT_VERSION);
    }

    #[test]
    fn injected_delimiters_do_not_collide() {
        // The hash's collision-resistance relies on every field emitting its own `name=` prefix,
        // NOT on delimiters being absent from values. Prove that newline/`=` injection in a free
        // field cannot impersonate another field: provider="p", model="qwen" must NOT collide
        // with provider="p\nmodel=qwen", model="" (the latter just gains an extra `model=` line).
        let honest = EmbeddingFingerprint::new(
            "p",
            "qwen",
            8,
            EmbeddingDtype::Float32,
            false,
            EmbeddingPooling::Unknown,
            None,
        );
        let injected = EmbeddingFingerprint::new(
            "p\nmodel=qwen",
            "",
            8,
            EmbeddingDtype::Float32,
            false,
            EmbeddingPooling::Unknown,
            None,
        );
        assert_ne!(honest.hash(), injected.hash());
    }

    #[test]
    fn hash_matches_golden_vector() {
        // Pins the exact payload byte-layout. A mutant that swaps two field labels or reorders
        // the payload lines would still produce a deterministic, collision-free hash and slip
        // past the `assert_ne!` change-detection tests — but it would diverge from this golden
        // digest. If the fingerprint format (or EMBEDDING_FINGERPRINT_VERSION) changes on
        // purpose, recompute and update this constant.
        const GOLDEN: &str = "c37dcd57be05776092b6e7e1468514d4e3c19f5b5eafa7ff0526864d8d0614fe";
        assert_eq!(sample().hash(), GOLDEN);
    }

    #[test]
    fn header_round_trips_through_serde() {
        let json = serde_json::to_string(&sample()).expect("serialize");
        assert!(json.contains("effectiveDim"));
        assert!(json.contains("instructionTemplate"));
        let restored: EmbeddingFingerprint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, sample());
        assert_eq!(restored.hash(), sample().hash());
    }
}
