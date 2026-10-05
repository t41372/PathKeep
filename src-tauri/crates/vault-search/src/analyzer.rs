//! Responsibilities: index normalized tokens and their complete Unicode prefixes.
//! Not responsible for: language normalization, candidate selection, or parsing queries.
//! Failure modes: a prefix cap hides long/rare expansions; internal names collide with
//! literal tokens; prefix aliases change phrase positions; prefix ranking uses a constant
//! instead of BM25. tests/ast.rs checks literal namespaces, prefix scores, and adjacency.

use tantivy::tokenizer::{Token, TokenStream, Tokenizer, WhitespaceTokenizer};

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
        }
    }
}
pub(crate) struct PrefixStream<'a> {
    source: <WhitespaceTokenizer as Tokenizer>::TokenStream<'a>,
    token: Token,
    literal: String,
    prefix_end: usize,
}
impl TokenStream for PrefixStream<'_> {
    fn advance(&mut self) -> bool {
        if self.prefix_end <= self.literal.len() {
            self.token.text.clear();
            self.token.text.push('^');
            self.token.text.push_str(&self.literal[..self.prefix_end]);
            self.token.offset_to = self.token.offset_from + self.prefix_end;
            self.prefix_end +=
                self.literal[self.prefix_end..].chars().next().map_or(1, char::len_utf8);
            return true;
        }
        if !self.source.advance() {
            return false;
        }
        self.token = self.source.token().clone();
        self.literal.clone_from(&self.token.text);
        self.token.text.insert(0, '=');
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
