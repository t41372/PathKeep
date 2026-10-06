//! Responsibilities: derive a sound mandatory ASCII fragment and advance its trigram mask.
//! Not responsible for: regex matching, sampling visits or keeping a cross-query result cache.
//! Failure modes: optional/alternative literals exclude matches; normalization changes a
//! non-ASCII fragment; stale indexes reject new pages. Tests below and regex integration tests.
use super::{Indexer, documents};
use regex_syntax::hir::{Hir, HirKind};
use std::time::Duration;
use vault_search::{Ast, MatchHandle};
use vault_store::{Db, Store};

pub(super) struct Prefilter {
    walker: Option<MatchHandle>,
    bits: Vec<u64>,
    pub seq: u64,
    pub processed: u64,
    pub pages: u64,
}
impl Prefilter {
    pub fn start(indexer: &Indexer, store: &Store, pattern: &str) -> anyhow::Result<Option<Self>> {
        let Some(fragment) = required_fragment(pattern) else { return Ok(None) };
        let seq = store.read(Db::Archive, documents::watermark)?;
        let session =
            match indexer.index()?.search(Ast::Substring { text: fragment }, Default::default()) {
                Ok(session) => session,
                Err(error) => {
                    if matches!(error, vault_search::Error::RebuildNeeded { .. }) {
                        indexer.request_repair();
                    }
                    return Ok(None);
                }
            };
        if session.last_applied_seq() != seq || session.initial_build_incomplete() {
            return Ok(None);
        }
        Ok(Some(Self {
            walker: Some(session.start_matches(false)?),
            bits: Vec::new(),
            seq,
            processed: 0,
            pages: 0,
        }))
    }
    pub fn step(&mut self, budget: Duration) -> vault_search::Result<bool> {
        let Some(walker) = &mut self.walker else { return Ok(true) };
        let step = walker.step(budget, 4096)?;
        self.processed = step.progress.documents_processed;
        for hit in step.hits {
            let word = hit.page_id as usize / 64;
            if self.bits.len() <= word {
                self.bits.resize(word + 1, 0);
            }
            self.bits[word] |= 1u64 << (hit.page_id % 64);
            self.pages += 1;
        }
        if !step.provisional {
            self.walker = None;
        }
        Ok(self.walker.is_none())
    }
    pub fn accepts(&self, page: u64) -> bool {
        self.bits.get(page as usize / 64).is_some_and(|bits| bits & (1u64 << (page % 64)) != 0)
    }
}
fn required_fragment(pattern: &str) -> Option<String> {
    let hir = regex_syntax::Parser::new().parse(pattern).ok()?;
    required(&hir).filter(|s| s.len() >= 3)
}
fn required(hir: &Hir) -> Option<String> {
    match hir.kind() {
        HirKind::Literal(literal) => std::str::from_utf8(&literal.0)
            .ok()?
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .max_by_key(|s| s.len())
            .map(|s| s[..s.len().min(32)].to_ascii_lowercase()),
        HirKind::Concat(children) => children.iter().filter_map(required).max_by_key(String::len),
        HirKind::Capture(capture) => required(&capture.sub),
        HirKind::Repetition(repetition) if repetition.min > 0 => required(&repetition.sub),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_or_alternative_text_is_never_a_required_fragment() {
        assert_eq!(required_fragment("(alpha|beta)"), None);
        assert_eq!(required_fragment("(?:needle)?"), None);
        assert_eq!(required_fragment("設定中心"), None);
        assert_eq!(required_fragment("needle.*suffix"), Some("suffix".into()));
        assert_eq!(required_fragment("(?:optional)?required"), Some("required".into()));
        assert_eq!(required_fragment("(?:needle)+"), Some("needle".into()));
    }
}
