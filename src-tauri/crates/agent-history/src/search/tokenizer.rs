//! Responsibilities: stream Unicode word tokens without silently dropping long words.
//! Not responsible for: stemming, fuzzy matching, linguistic translation or ranking.
//! Han, kana and Hangul scalars are individual terms, so short CJK queries can find
//! text inside an unspaced sentence. Long words use the same hash at index and query.

use sha2::{Digest, Sha256};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

#[derive(Clone, Default)]
pub(super) struct HistoryTokenizer;

impl Tokenizer for HistoryTokenizer {
    type TokenStream<'a> = HistoryTokens<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        HistoryTokens { text, cursor: 0, next_position: 0, token: Token::default() }
    }
}

pub(super) struct HistoryTokens<'a> {
    text: &'a str,
    cursor: usize,
    next_position: usize,
    token: Token,
}

impl TokenStream for HistoryTokens<'_> {
    fn advance(&mut self) -> bool {
        while self.cursor < self.text.len() {
            let start = self.cursor;
            let character = self.text[start..].chars().next().unwrap();
            self.cursor += character.len_utf8();
            if !character.is_alphanumeric() && character != '_' {
                continue;
            }
            if !is_cjk(character) {
                while let Some(next) = self.text[self.cursor..].chars().next() {
                    if is_cjk(next) || (!next.is_alphanumeric() && next != '_') {
                        break;
                    }
                    self.cursor += next.len_utf8();
                }
            }
            self.token.offset_from = start;
            self.token.offset_to = self.cursor;
            self.token.position = self.next_position;
            self.next_position += 1;
            self.token.text = encode_word(&self.text[start..self.cursor]);
            return true;
        }
        false
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

fn is_cjk(character: char) -> bool {
    matches!(character as u32,
        0x2e80..=0x2fff | 0x3040..=0x30ff | 0x31f0..=0x31ff |
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xac00..=0xd7af |
        0xf900..=0xfaff | 0x20000..=0x323af)
}

fn encode_word(word: &str) -> String {
    // Tantivy silently discards terms above its own byte limit. A fixed-size
    // namespace avoids that loss without retaining a second copy of a huge word.
    let mut small = String::new();
    let mut hash = Sha256::new();
    let mut length = 0;
    for character in word.chars().flat_map(char::to_lowercase) {
        let mut buffer = [0; 4];
        let bytes = character.encode_utf8(&mut buffer).as_bytes();
        hash.update(bytes);
        length += bytes.len();
        if length <= 128 {
            small.push(character);
        }
    }
    if length <= 128 {
        format!("r:{small}")
    } else {
        format!("h:{length}:{}", hex::encode(hash.finalize()))
    }
}

pub(super) fn query_terms(text: &str) -> Vec<String> {
    let mut tokenizer = HistoryTokenizer;
    let mut stream = tokenizer.token_stream(text);
    let mut terms = Vec::new();
    while stream.advance() {
        let term = stream.token().text.clone();
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    terms
}
