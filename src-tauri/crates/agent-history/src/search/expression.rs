//! Match readable parts, preserving their UTF-8 coordinates. Index terms are only candidates.
use super::{stream::Role, tokenizer::HistoryTokenizer};
use crate::{
    Error, Result,
    read_model::{PartKind, RecordContext, parts},
};
use regex::{Regex, RegexBuilder};
use std::{
    collections::{HashSet, VecDeque},
    ops::Range,
};
use tantivy::tokenizer::{TokenStream, Tokenizer};
use vault_store::CancelToken;

pub(crate) struct Part {
    pub role: Role,
    pub kind: PartKind,
    pub text: String,
}

pub(crate) fn readable(
    event: &crate::NativeEvent,
    harness: crate::Harness,
    format: Option<i64>,
) -> Vec<Part> {
    let context = if harness == crate::Harness::Deepseek {
        RecordContext::Deepseek { version: format.and_then(|v| v.try_into().ok()) }
    } else {
        RecordContext::new(harness)
    };
    parts::render(&context, event)
        .into_iter()
        .map(|p| {
            let role = match p.kind {
                PartKind::Thinking => Role::Thinking,
                PartKind::CallInput | PartKind::CallResult => Role::Tool,
                PartKind::Text if event.role.as_deref() == Some("user") => Role::User,
                PartKind::Text => Role::Agent,
            };
            let text = if role == Role::Agent { markdown(&p.text) } else { p.text };
            Part { role, kind: p.kind, text }
        })
        .collect()
}
fn markdown(src: &str) -> String {
    use pulldown_cmark::{Event, Parser, TagEnd};
    let mut out = String::new();
    for event in Parser::new(src) {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak
            | Event::HardBreak
            | Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::CodeBlock,
            ) => out.push('\n'),
            _ => {}
        }
    }
    out.truncate(out.trim_end_matches('\n').len());
    out
}

pub(super) enum Expression {
    Words(Vec<String>),
    Phrase(Vec<String>),
    Regex(Regex),
}
impl Expression {
    pub fn parse(query: &str) -> Result<Self> {
        let q = query.trim();
        if q.len() > 4096 {
            return Err(Error::InvalidInput("search query exceeds 4096 bytes".into()));
        }
        if let Some(regex) = q.strip_prefix('/').and_then(|s| s.strip_suffix('/')) {
            return Ok(Self::Regex(
                RegexBuilder::new(regex)
                    .case_insensitive(true)
                    .build()
                    .map_err(|_| Error::InvalidRegex)?,
            ));
        }
        let phrase = q.strip_prefix('"').and_then(|s| s.strip_suffix('"'));
        let mut analyzer = HistoryTokenizer;
        let mut stream = analyzer.token_stream(phrase.unwrap_or(q));
        let mut words = vec![];
        while stream.advance() {
            words.push(stream.token().text.clone());
        }
        if words.is_empty() {
            return Err(Error::InvalidInput("enter a word, phrase or /regex/".into()));
        }
        Ok(if phrase.is_some() {
            Self::Phrase(words)
        } else {
            words.sort();
            words.dedup();
            Self::Words(words)
        })
    }
    pub fn terms(&self) -> &[String] {
        match self {
            Self::Regex(_) => &[],
            Self::Words(w) | Self::Phrase(w) => w,
        }
    }
    pub fn visit(
        &self,
        parts: &[(usize, &str)],
        cancel: &CancelToken,
        mut hit: impl FnMut(usize, Range<usize>) -> Result<()>,
    ) -> Result<()> {
        let check = || cancel.check().map_err(|_| Error::Cancelled);
        if let Self::Words(words) = self
            && words.len() > 1
        {
            let mut found = HashSet::new();
            for (_, text) in parts {
                let mut analyzer = HistoryTokenizer;
                let mut tokens = analyzer.token_stream(text);
                while tokens.advance() {
                    let t = tokens.token();
                    if t.position % 256 == 0 {
                        check()?
                    }
                    if let Ok(i) = words.binary_search(&t.text) {
                        found.insert(i);
                    }
                    if found.len() == words.len() {
                        break;
                    }
                }
                if found.len() == words.len() {
                    break;
                }
            }
            if found.len() != words.len() {
                return Ok(());
            }
        }
        for &(part, text) in parts {
            check()?;
            if let Self::Regex(re) = self {
                for m in re.find_iter(text) {
                    check()?;
                    hit(part, m.range())?;
                }
                continue;
            }
            let mut analyzer = HistoryTokenizer;
            let mut tokens = analyzer.token_stream(text);
            let mut recent = VecDeque::new();
            while tokens.advance() {
                let t = tokens.token();
                if t.position % 256 == 0 {
                    check()?
                }
                match self {
                    Self::Words(w) if w.binary_search(&t.text).is_ok() => {
                        hit(part, t.offset_from..t.offset_to)?
                    }
                    Self::Phrase(w) => {
                        recent.push_back((t.text.clone(), t.offset_from));
                        if recent.len() > w.len() {
                            recent.pop_front();
                        }
                        if recent.len() == w.len() && recent.iter().map(|(term, _)| term).eq(w) {
                            hit(part, recent[0].1..t.offset_to)?
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
