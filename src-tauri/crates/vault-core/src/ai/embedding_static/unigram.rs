//! Responsibilities: compact byte-prefix lookup and inference for Unigram models.
//! Not responsible for normalization, added tokens, training, or embedding pooling.
//!
//! Segmentation is ported from Hugging Face tokenizers 0.22.2 (Apache-2.0),
//! src/models/unigram/model.rs: encode_optimized and Model::tokenize. Preserve character traversal,
//! shortest-prefix-first visits, strict score comparisons, f64 arithmetic, fuse_unk=true, and
//! byte fallback. Only prefix lookup and storage change; backtracking retains spans instead of
//! copying strings. Empty or duplicate pieces, which upstream accepts silently, fail to load.

use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokenizers::{AddedToken, Model, Token, Trainer};

#[derive(Deserialize)]
struct UnigramFile<'a> {
    #[serde(rename = "type")]
    kind: UnigramKind,
    #[serde(borrow)]
    vocab: Vec<Piece<'a>>,
    unk_id: Option<u32>,
    #[serde(default)]
    byte_fallback: bool,
}

// Borrow unescaped JSON strings while loading. Allocating 500,000 temporary Strings would leave
// allocator pages resident even after the packed vocabulary replaces them.
#[derive(Deserialize)]
struct Piece<'a>(#[serde(borrow)] Cow<'a, str>, f64);

#[derive(Deserialize)]
enum UnigramKind {
    Unigram,
}

/// A read-only vocabulary whose memory is proportional to piece bytes and piece count.
/// Sorted positions and original model IDs are distinct: sorting must never change matrix rows.
pub(super) struct CompactUnigram {
    bytes: Vec<u8>,
    offsets: Vec<u32>,
    scores: Vec<f64>,
    ids: Vec<u32>,
    positions_by_id: Vec<u32>,
    unk_id: Option<u32>,
    byte_fallback: bool,
    min_score: f64,
}

impl<'de> Deserialize<'de> for CompactUnigram {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(UnigramFile::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<UnigramFile<'_>> for CompactUnigram {
    type Error = String;

    fn try_from(file: UnigramFile<'_>) -> Result<Self, Self::Error> {
        let UnigramFile { kind: UnigramKind::Unigram, vocab, unk_id, byte_fallback } = file;
        if vocab.len() > u32::MAX as usize || unk_id.is_some_and(|id| id as usize >= vocab.len()) {
            return Err("Unigram vocabulary size or unk_id is out of range".into());
        }
        let byte_len = vocab.iter().map(|Piece(piece, _)| piece.len()).sum::<usize>();
        if byte_len > u32::MAX as usize {
            return Err("Unigram piece buffer exceeds u32 offsets".into());
        }
        let mut pieces: Vec<_> = vocab.into_iter().enumerate().collect();
        pieces.sort_unstable_by(|a, b| a.1.0.cmp(&b.1.0));
        let mut model = Self {
            bytes: Vec::with_capacity(byte_len),
            offsets: Vec::with_capacity(pieces.len() + 1),
            scores: Vec::with_capacity(pieces.len()),
            ids: Vec::with_capacity(pieces.len()),
            positions_by_id: vec![0; pieces.len()],
            unk_id,
            byte_fallback,
            min_score: f64::INFINITY,
        };
        for (id, Piece(piece, score)) in pieces {
            if !score.is_finite() || piece.is_empty() {
                return Err("Unigram pieces must be nonempty with finite scores".into());
            }
            if !model.ids.is_empty() && model.piece(model.ids.len() - 1) == piece.as_bytes() {
                return Err("duplicate Unigram piece".into());
            }
            model.positions_by_id[id] = model.ids.len() as u32;
            model.offsets.push(model.bytes.len() as u32);
            model.bytes.extend_from_slice(piece.as_bytes());
            model.scores.push(score);
            model.ids.push(id as u32);
            model.min_score = model.min_score.min(score);
        }
        model.offsets.push(model.bytes.len() as u32);
        Ok(model)
    }
}

impl CompactUnigram {
    fn piece(&self, position: usize) -> &[u8] {
        // During construction the final offset has not yet been appended.
        let end = self.offsets.get(position + 1).copied().unwrap_or(self.bytes.len() as u32);
        &self.bytes[self.offsets[position] as usize..end as usize]
    }

    /// Narrows the range sharing a prefix by one byte at a time. Terminal pieces precede their
    /// extensions in byte order, matching the upstream trie's shortest-prefix-first iteration.
    fn prefixes(&self, input: &[u8], mut visit: impl FnMut(usize, usize)) {
        let mut start = 0;
        let mut end = self.ids.len();
        for (depth, byte) in input.iter().enumerate() {
            let offsets = &self.offsets[start..end];
            // Terminal pieces were removed on the preceding iteration, so every remaining piece
            // has a byte at this depth. Bytes past its end must never participate in the search.
            let next_byte = |offset: &u32| self.bytes[*offset as usize + depth];
            let lower = offsets.partition_point(|offset| next_byte(offset) < *byte);
            let upper = offsets.partition_point(|offset| next_byte(offset) <= *byte);
            end = start + upper;
            start += lower;
            if start == end {
                break;
            }
            let length = depth + 1;
            if self.piece(start).len() == length {
                visit(start, length);
                start += 1;
            }
        }
    }

    fn segment(&self, sentence: &str) -> tokenizers::Result<Vec<Token>> {
        #[derive(Clone, Default)]
        struct BestPathNode {
            id: u32,
            best_path_score: f64,
            starts_at: Option<usize>,
        }
        let size = sentence.len();
        let unk_score = self.min_score - 10.0;
        let mut best_path_ends_at = vec![BestPathNode::default(); size + 1];
        for (starts_at, character) in sentence.char_indices() {
            let best_path_score_till_here = best_path_ends_at[starts_at].best_path_score;
            let mblen = character.len_utf8();
            let mut has_single_node = false;
            self.prefixes(&sentence.as_bytes()[starts_at..], |position, length| {
                let target_node = &mut best_path_ends_at[starts_at + length];
                let candidate_best_path_score = self.scores[position] + best_path_score_till_here;
                if target_node.starts_at.is_none()
                    || candidate_best_path_score > target_node.best_path_score
                {
                    target_node.best_path_score = candidate_best_path_score;
                    target_node.starts_at = Some(starts_at);
                    target_node.id = self.ids[position];
                }
                if length == mblen {
                    has_single_node = true;
                }
            });
            if !has_single_node {
                let target_node = &mut best_path_ends_at[starts_at + mblen];
                let candidate_best_path_score = unk_score + best_path_score_till_here;
                if target_node.starts_at.is_none()
                    || candidate_best_path_score > target_node.best_path_score
                {
                    target_node.best_path_score = candidate_best_path_score;
                    target_node.starts_at = Some(starts_at);
                    target_node.id = self.unk_id.ok_or("Unigram is missing unk_id")?;
                }
            }
        }
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut ends_at = size;
        let mut previous_was_unk = false;
        while ends_at > 0 {
            let node = &best_path_ends_at[ends_at];
            let starts_at = node.starts_at.expect("every character has a known or unknown path");
            let is_unk = Some(node.id) == self.unk_id;
            if is_unk && previous_was_unk {
                spans.last_mut().expect("previous unknown span").0 = starts_at;
            } else {
                spans.push((starts_at, ends_at));
            }
            previous_was_unk = is_unk;
            ends_at = starts_at;
        }
        let mut tokens = Vec::with_capacity(spans.len());
        for (start, end) in spans.into_iter().rev() {
            let piece = &sentence[start..end];
            // Upstream resolves the fused string again, rather than using the Viterbi node's ID.
            if let Some(id) = self.token_to_id(piece) {
                tokens.push(Token::new(id, piece.to_owned(), (start, end)));
                continue;
            }
            if let Some(bytes) = self.byte_tokens(piece, (start, end)) {
                tokens.extend(bytes);
                continue;
            }
            let id = self.unk_id.ok_or("Unigram is missing unk_id")?;
            tokens.push(Token::new(id, piece.to_owned(), (start, end)));
        }
        Ok(tokens)
    }

    /// Spells an unknown piece as `<0xNN>` tokens, each carrying the whole piece's offsets, when
    /// the model has byte fallback and every byte token exists; otherwise the piece stays unknown.
    fn byte_tokens(&self, piece: &str, offsets: (usize, usize)) -> Option<Vec<Token>> {
        if !self.byte_fallback {
            return None;
        }
        piece
            .bytes()
            .map(|byte| {
                let name = format!("<0x{byte:02X}>");
                self.token_to_id(&name).map(|id| Token::new(id, name, offsets))
            })
            .collect()
    }
}

impl Model for CompactUnigram {
    type Trainer = InferenceOnly;

    fn tokenize(&self, sequence: &str) -> tokenizers::Result<Vec<Token>> {
        self.segment(sequence)
    }

    fn token_to_id(&self, token: &str) -> Option<u32> {
        let mut start = 0;
        let mut end = self.ids.len();
        while start < end {
            let middle = start + (end - start) / 2;
            match self.piece(middle).cmp(token.as_bytes()) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Some(self.ids[middle]),
            }
        }
        None
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        let position = *self.positions_by_id.get(id as usize)? as usize;
        Some(String::from_utf8(self.piece(position).to_vec()).expect("UTF-8 vocabulary"))
    }

    fn get_vocab(&self) -> HashMap<String, u32> {
        self.ids
            .iter()
            .enumerate()
            .map(|(position, id)| {
                (String::from_utf8(self.piece(position).to_vec()).expect("UTF-8 vocabulary"), *id)
            })
            .collect()
    }

    fn get_vocab_size(&self) -> usize {
        self.ids.len()
    }

    fn save(&self, _folder: &Path, _prefix: Option<&str>) -> tokenizers::Result<Vec<PathBuf>> {
        Err("compact Unigram is inference-only; retain the source tokenizer.json".into())
    }

    fn get_trainer(&self) -> Self::Trainer {
        InferenceOnly
    }
}

/// Makes accidental use of a loaded embedding vocabulary for training fail explicitly.
pub(super) struct InferenceOnly;

impl Trainer for InferenceOnly {
    type Model = CompactUnigram;
    fn should_show_progress(&self) -> bool {
        false
    }
    fn train(&self, _model: &mut Self::Model) -> tokenizers::Result<Vec<AddedToken>> {
        Err("compact Unigram is inference-only".into())
    }
    fn feed<I, S, F>(&mut self, _iterator: I, _process: F) -> tokenizers::Result<()>
    where
        I: Iterator<Item = S> + Send,
        S: AsRef<str> + Send,
        F: Fn(&str) -> tokenizers::Result<Vec<String>> + Sync,
    {
        Err("compact Unigram is inference-only".into())
    }
}
