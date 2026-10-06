//! Responsibilities: index normalized tokens and their complete Unicode prefixes.
//! Not responsible for: language normalization, candidate selection, or parsing queries.
//! Failure modes: a prefix cap hides long/rare expansions; internal names collide with
//! literal tokens; prefix aliases change phrase positions; prefix ranking uses a constant
//! instead of BM25. tests/ast.rs checks literal namespaces, prefix scores, and adjacency.

use sha2::{Digest, Sha256};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer, WhitespaceTokenizer};

const RAW_TOKEN_BYTES: usize = 128;
/// Fixed-size encodings keep arbitrarily long normalized tokens searchable without
/// quadratic prefix bytes or Tantivy's silent token-length truncation. Raw/hash
/// namespaces are disjoint, including user literals that resemble either namespace.
pub(crate) fn indexed_token(token: &str, prefix: bool) -> String {
    let kind = if prefix { '^' } else { '=' };
    if token.len() <= RAW_TOKEN_BYTES {
        format!("{kind}r{token}")
    } else {
        format!("{kind}h{}:{}", token.len(), hex::encode(Sha256::digest(token.as_bytes())))
    }
}

/// Two namespaces prevent user tokens such as "^42" from colliding with aliases.
/// All aliases occupy the original token position, preserving phrase adjacency.
#[derive(Clone, Default)]
pub(crate) struct PrefixTokenizer {
    source: WhitespaceTokenizer,
}
impl Tokenizer for PrefixTokenizer {
    type TokenStream<'a> = PrefixStream<'a>;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        PrefixStream {
            source: self.source.token_stream(text),
            token: Token::default(),
            literal: String::new(),
            prefix_end: 1,
            prefix_hash: Sha256::new(),
        }
    }
}
pub(crate) struct PrefixStream<'a> {
    source: <WhitespaceTokenizer as Tokenizer>::TokenStream<'a>,
    token: Token,
    literal: String,
    prefix_end: usize,
    prefix_hash: Sha256,
}
impl TokenStream for PrefixStream<'_> {
    fn advance(&mut self) -> bool {
        if self.prefix_end <= self.literal.len() {
            self.token.text = if self.prefix_end <= RAW_TOKEN_BYTES {
                format!("^r{}", &self.literal[..self.prefix_end])
            } else {
                format!(
                    "^h{}:{}",
                    self.prefix_end,
                    hex::encode(self.prefix_hash.clone().finalize())
                )
            };
            self.token.offset_to = self.token.offset_from + self.prefix_end;
            let next = self.literal[self.prefix_end..].chars().next().map_or(1, char::len_utf8);
            if self.prefix_end < self.literal.len() {
                self.prefix_hash
                    .update(&self.literal.as_bytes()[self.prefix_end..self.prefix_end + next]);
            }
            self.prefix_end += next;
            return true;
        }
        if !self.source.advance() {
            return false;
        }
        self.token = self.source.token().clone();
        self.literal.clone_from(&self.token.text);
        self.token.text = indexed_token(&self.literal, false);
        self.prefix_hash = Sha256::new();
        self.prefix_end = 0;
        true
    }
    fn token(&self) -> &Token {
        &self.token
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

/// Character positions preserve exact compact substrings. Tantivy's general n-gram
/// tokenizer puts every gram at position zero, which cannot prove adjacency.
#[derive(Clone, Default)]
pub(crate) struct TrigramTokenizer;
impl Tokenizer for TrigramTokenizer {
    type TokenStream<'a> = TrigramStream<'a>;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        TrigramStream { chars: text.char_indices(), token: Token::default(), position: 0 }
    }
}
pub(crate) struct TrigramStream<'a> {
    chars: std::str::CharIndices<'a>,
    token: Token,
    position: usize,
}
impl TokenStream for TrigramStream<'_> {
    fn advance(&mut self) -> bool {
        let mut chars = self.chars.clone();
        let Some((first, a)) = chars.next() else { return false };
        let Some((_, b)) = chars.next() else { return false };
        let Some((last, c)) = chars.next() else { return false };
        self.token.text.clear();
        self.token.text.extend([a, b, c]);
        self.token.offset_from = first;
        self.token.offset_to = last + c.len_utf8();
        self.token.position = self.position;
        self.position += 1;
        self.chars.next();
        true
    }
    fn token(&self) -> &Token {
        &self.token
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}
