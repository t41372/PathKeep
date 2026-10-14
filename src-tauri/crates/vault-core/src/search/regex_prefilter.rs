//! Responsibilities: derive a sound mandatory ASCII fragment and list every page that
//! could contain it: the index snapshot's matches plus the pages written since.
//! Not responsible for: regex matching, sampling visits or keeping a cross-query result cache.
//! Failure modes: optional/alternative literals exclude matches; normalization changes a
//! non-ASCII fragment; pages written after the snapshot are missed; the indexer prunes the
//! outbox between pinning the snapshot and reading it. Tests below and regex integration tests.
use super::{Indexer, documents};
use regex_syntax::hir::{Hir, HirKind};
use std::time::Duration;
use vault_search::{Ast, MatchHandle};
use vault_store::{Db, Store};

/// How often to pin a newer snapshot when the indexer pruned the outbox past the pinned one.
const PIN_ATTEMPTS: usize = 8;

pub(super) struct Prefilter {
    walker: Option<MatchHandle>,
    bits: Vec<u64>,
    /// Pages whose search documents changed after the snapshot; checked whatever their text.
    written: Vec<u64>,
    pub processed: u64,
}
impl Prefilter {
    /// None when the pattern has no usable fragment or the index can't vouch for every page.
    pub fn start(indexer: &Indexer, store: &Store, pattern: &str) -> anyhow::Result<Option<Self>> {
        let Some(fragment) = required_fragment(pattern) else { return Ok(None) };
        for _ in 0..PIN_ATTEMPTS {
            let session = match indexer
                .index()?
                .search(Ast::Substring { text: fragment.clone() }, Default::default())
            {
                Ok(session) => session,
                Err(error) => {
                    if matches!(error, vault_search::Error::RebuildNeeded { .. }) {
                        indexer.request_repair();
                    }
                    return Ok(None);
                }
            };
            if session.initial_build_incomplete() {
                return Ok(None);
            }
            let pinned = session.last_applied_seq();
            // The indexer publishes a snapshot before pruning the outbox up to it, so the
            // outbox holds every change after `pinned` unless a newer snapshot already exists.
            let (complete, written) = store.read(Db::Archive, move |s| {
                let watermark = documents::watermark(s)?;
                let oldest = s.query_one(
                    "SELECT COALESCE(MIN(seq),0) FROM search_outbox WHERE seq>?1",
                    [pinned as i64],
                    |r| Ok(r.get::<i64>(0)? as u64),
                )?;
                let written = s.query(
                    "SELECT DISTINCT page_id FROM search_outbox WHERE seq>?1",
                    [pinned as i64],
                    |r| Ok(r.get::<i64>(0)? as u64),
                )?;
                Ok((watermark <= pinned || oldest == pinned + 1, written))
            })?;
            if complete {
                return Ok(Some(Self {
                    walker: Some(session.start_matches(false)?),
                    bits: Vec::new(),
                    written,
                    processed: 0,
                }));
            }
        }
        Ok(None)
    }
    /// True once every match of the snapshot has been read.
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
        }
        if !step.provisional {
            self.walker = None;
        }
        Ok(self.walker.is_none())
    }
    /// Every page that could match, ascending.
    pub fn candidates(&self) -> Vec<u64> {
        let mut pages = self
            .bits
            .iter()
            .enumerate()
            .flat_map(|(word, bits)| {
                (0..64)
                    .filter(move |bit| bits & (1u64 << bit) != 0)
                    .map(move |bit| (word * 64 + bit) as u64)
            })
            .chain(self.written.iter().copied())
            .collect::<Vec<_>>();
        pages.sort_unstable();
        pages.dedup();
        pages
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
