//! Responsibilities: retain tokenizers' preprocessing and added-token behavior with a compact Unigram.
//! Not responsible for pooling, model downloads, or tokenizer training.

use super::unigram::CompactUnigram;
use serde::Deserialize;
use tokenizers::{
    DecoderWrapper, Encoding, NormalizerWrapper, PostProcessorWrapper, PreTokenizerWrapper,
};

type CompactTokenizer = tokenizers::TokenizerImpl<
    CompactUnigram,
    NormalizerWrapper,
    PreTokenizerWrapper,
    PostProcessorWrapper,
    DecoderWrapper,
>;

/// Keeps the static model's tokenizer resident without tokenizers' per-node Unigram hash maps.
/// Other model types retain their existing tokenizers implementation and pooling behavior.
pub struct StaticTokenizer(Backend);

enum Backend {
    Compact(Box<CompactTokenizer>),
    Other(Box<tokenizers::Tokenizer>),
}

impl StaticTokenizer {
    /// Uses the crate's tokenizer deserializer, substituting only the Unigram model type.
    /// The header pass skips vocabulary data without allocating it. An invalid Unigram fails here;
    /// it never falls back to constructing the upstream trie, even transiently.
    pub fn from_bytes(bytes: &[u8]) -> tokenizers::Result<Self> {
        #[derive(Deserialize)]
        struct Header {
            model: Kind,
        }
        #[derive(Deserialize)]
        struct Kind {
            #[serde(rename = "type")]
            name: String,
        }
        let header: Header = serde_json::from_slice(bytes)?;
        let backend = if header.model.name == "Unigram" {
            Backend::Compact(Box::new(serde_json::from_slice(bytes)?))
        } else {
            Backend::Other(Box::new(tokenizers::Tokenizer::from_bytes(bytes)?))
        };
        Ok(Self(backend))
    }

    /// Preserves the original encoding boundary, including added tokens and byte offsets.
    pub fn encode(&self, text: &str, add_special_tokens: bool) -> tokenizers::Result<Encoding> {
        match &self.0 {
            Backend::Compact(tokenizer) => tokenizer.encode(text, add_special_tokens),
            Backend::Other(tokenizer) => tokenizer.encode(text, add_special_tokens),
        }
    }

    /// Resolves string unk declarations for the existing non-Unigram pooling policy.
    pub fn token_to_id(&self, token: &str) -> Option<u32> {
        match &self.0 {
            Backend::Compact(tokenizer) => tokenizer.token_to_id(token),
            Backend::Other(tokenizer) => tokenizer.token_to_id(token),
        }
    }
}

#[cfg(test)]
impl From<tokenizers::Tokenizer> for StaticTokenizer {
    fn from(tokenizer: tokenizers::Tokenizer) -> Self {
        Self(Backend::Other(Box::new(tokenizer)))
    }
}
