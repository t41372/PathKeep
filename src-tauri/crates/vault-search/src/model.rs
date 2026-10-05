//! Responsibilities: Define normalized page documents, batches, and resource settings.
//! Not responsible for: Text normalization or canonical visit eligibility.
//! Failure modes: Field order or weights drift; absent profiles become matches; invalid
//! writer settings reach Tantivy; oversized terms are silently dropped. Tested in
//! contracts and recovery.

/// Matches the canonical projection's field order; discriminants are schema contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum TextField {
    Url,
    Title,
    SearchTerms,
    NormalizedUrl,
    NormalizedTitle,
    NormalizedSearchTerms,
    CjkGrams,
    Enrichment,
    Notes,
    Tags,
}
impl TextField {
    pub const ALL: [Self; 10] = [
        Self::Url,
        Self::Title,
        Self::SearchTerms,
        Self::NormalizedUrl,
        Self::NormalizedTitle,
        Self::NormalizedSearchTerms,
        Self::CjkGrams,
        Self::Enrichment,
        Self::Notes,
        Self::Tags,
    ];
    pub const NAMES: [&'static str; 10] = [
        "url",
        "title",
        "search_terms",
        "normalized_url",
        "normalized_title",
        "normalized_search_terms",
        "cjk_grams",
        "enrichment_text",
        "notes_text",
        "tags_text",
    ];
    pub const WEIGHTS: [f32; 10] = [6., 12., 4., 5., 10., 4., 2., 7., 9., 6.];
}

/// Full replacement values from canonical storage. Text is already normalized and
/// whitespace-delimited by vault-core; this crate never folds or segments language.
#[derive(Clone, Debug)]
pub struct PageDocument {
    pub page_id: u64,
    pub text: [String; 10],
    pub compact_text: String,
    pub newest_visit_ms: i64,
    pub domain: String,
    pub profile_ids: Vec<u64>,
}
impl PageDocument {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        for (name, text) in TextField::NAMES.into_iter().zip(&self.text) {
            if text
                .split_ascii_whitespace()
                .any(|token| token.len() >= tantivy::tokenizer::MAX_TOKEN_LEN)
            {
                return Err(crate::Error::Input(format!(
                    "page {}: {name} has a token too long for Tantivy",
                    self.page_id
                )));
            }
        }
        Ok(())
    }

    /// Starts an empty projection so callers can fill all applicable fields explicitly.
    pub fn new(page_id: u64) -> Self {
        Self {
            page_id,
            text: Default::default(),
            compact_text: String::new(),
            newest_visit_ms: 0,
            domain: String::new(),
            profile_ids: vec![],
        }
    }
}

/// A complete prefix of the caller's outbox. Deletes precede upserts; the final
/// upsert for an id wins. The caller must not advance last_seq past unapplied changes.
#[derive(Debug)]
pub struct Batch {
    pub upserts: Vec<PageDocument>,
    pub deletes: Vec<u64>,
    pub last_seq: u64,
}

/// Controls visibility without an asynchronous reload thread or hidden read-side writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReloadPolicy {
    #[default]
    AfterCommit,
    Manual,
}

/// Explicit resource settings. The writer budget is total arena memory, not total RSS.
#[derive(Clone, Debug)]
pub struct Config {
    /// Total indexing arena budget, divided among writer_threads.
    pub writer_memory_bytes: usize,
    /// Fixed indexing concurrency; unlike Tantivy defaults, independent of host cores.
    pub writer_threads: usize,
    /// Decrypted blocks and shared ranges retained by EncryptedDirectory.
    pub cache_bytes: usize,
    /// Chooses when new sessions see committed writes. Existing sessions stay pinned.
    pub reload_policy: ReloadPolicy,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            writer_memory_bytes: 64 * 1024 * 1024,
            writer_threads: 2,
            cache_bytes: 128 * 1024 * 1024,
            reload_policy: ReloadPolicy::AfterCommit,
        }
    }
}
impl Config {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.writer_threads == 0 || self.writer_memory_bytes / self.writer_threads < 15_000_000 {
            return Err(crate::Error::Input(
                "writer requires at least 15,000,000 bytes per thread".into(),
            ));
        }
        Ok(())
    }
}
