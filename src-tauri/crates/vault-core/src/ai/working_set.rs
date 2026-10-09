//! Responsibilities: enumerate canonical pages and score star, recency, annotation and frequency signals.
//! Not responsible for: embeddings, content fetching or canonical-URL normalization.
//! Failure modes: a pool cap hides pages, variants duplicate candidates, a sparse page loses
//! continuation, or a scalar signal query groups the whole archive once per candidate.
//!
//! Reads seek the next canonical representative by page ID and return an ID cursor.
//! Signal scores become queue priorities; they never impose a total candidate cap.
//! Native paging and plan tests cover plaintext and encrypted archives past both former caps.

use crate::models::{ReembedEstimate, ReembedScope};
#[cfg(test)]
use crate::visit_taxonomy::{normalize_visit_url, registrable_domain_for_url};
use anyhow::Result;
#[cfg(test)]
use std::collections::HashMap;

/// Tunable weights + window for the working-set ranking (05 §8: user can configure each dimension).
///
/// `starred` is the TOP weight (declared signal, 05 §4) — a starred page outranks one that is merely
/// recent or frequent. The weights are additive per signal so a page hitting multiple signals
/// (starred AND frequent) ranks above one hitting a single signal, which is the Pareto intent. The
/// recency window is the "recent" cutoff in months (12–24 per 05 §8); a visit older than the window
/// contributes no recency score. Defaults are a sane starting point; the heavy-tier / enrichment
/// settings UI (W-AI-9) lets the user toggle dimensions + set the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkingSetConfig {
    /// Score added when the page (or its domain) is starred — the highest-confidence member.
    pub starred_weight: f64,
    /// Score added when the page was visited within the recency window.
    pub recent_weight: f64,
    /// Score added when the page has a note or any tag (a deliberate keep signal).
    pub annotated_weight: f64,
    /// Score multiplier applied to the (log-damped) visit count — the refind/habit signal.
    pub frequency_weight: f64,
    /// Recency window in months; a last-visit older than this contributes no recency score.
    pub recency_window_months: u32,
}

impl Default for WorkingSetConfig {
    fn default() -> Self {
        Self {
            starred_weight: 100.0,
            recent_weight: 10.0,
            annotated_weight: 25.0,
            frequency_weight: 5.0,
            recency_window_months: 18,
        }
    }
}

/// The four declared signals observed for one unique-content candidate.
///
/// A pure value object so the SCORING is unit-tested independent of SQL: a test constructs the
/// signals directly and asserts the ranking, and a separate test drives the bounded SQL gather.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CandidateSignals {
    /// The page (canonical URL) or its domain is starred.
    pub starred: bool,
    /// The page was visited within the recency window.
    pub recent: bool,
    /// The page has a note or at least one tag.
    pub annotated: bool,
    /// The page's total visit count (the refind/frequency signal).
    pub visit_count: u32,
}

/// One ranked working-set candidate: a unique canonical URL plus its score + the signals behind it.
///
/// `canonical_url` is the dedup unit (one entry per page, 05 §1), so the heavy tier / enrichment fan
/// the decision out to every visit sharing it. `score` is the additive priority; `signals` is kept so
/// a consumer (or a UI) can explain WHY a page was selected (PME transparency).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkingSetCandidate {
    /// Canonical URL (the dedup unit) the candidate represents.
    pub canonical_url: String,
    /// A stored URL variant for fetching. Canonical identity is resolved through Archive pages.
    pub raw_url: Option<String>,
    /// Registrable-ish domain (via the shared `url_domain`) for grouping/display.
    pub domain: String,
    /// Most-recent page title seen for this canonical URL, when known.
    pub title: Option<String>,
    /// Additive priority score (higher = embed/fetch first).
    pub score: f64,
    /// The signals that produced the score (kept for explainability).
    pub signals: CandidateSignals,
}

/// Computes the additive priority score for one candidate's signals under a config.
///
/// PURE (no I/O) so the ranking policy is unit-tested + mutation-hardened directly. The score sums
/// each ACTIVE signal's weight, so multi-signal pages outrank single-signal ones; frequency is
/// log-damped (`ln(1 + visit_count)`) so a page with 5000 visits does not swamp the additive signals
/// — a heavily-revisited page ranks high but a STARRED page (top weight) still wins, which is the
/// declared-signal-first intent of 05 §4. A candidate with NO active signal scores 0 (it is not a
/// working-set member).
pub fn score_candidate(signals: &CandidateSignals, config: &WorkingSetConfig) -> f64 {
    let mut score = 0.0;
    if signals.starred {
        score += config.starred_weight;
    }
    if signals.recent {
        score += config.recent_weight;
    }
    if signals.annotated {
        score += config.annotated_weight;
    }
    if signals.visit_count > 0 {
        score += config.frequency_weight * (1.0 + signals.visit_count as f64).ln();
    }
    score
}

/// A bounded traversal page. The queue uses scores as priorities after enumeration.
#[derive(Debug, Clone)]
pub struct WorkingSetPage {
    pub items: Vec<WorkingSetCandidate>,
    pub next_cursor: Option<i64>,
}

/// Enumerates canonical pages without limiting any signal's coverage.
/// `now_ms` is Unix milliseconds. Reuse the ID cursor even when a page has no
/// positive-scoring candidates; limits bound one read, never the reachable set.
pub fn select_working_set(
    vault: &crate::Vault,
    config: &WorkingSetConfig,
    now_ms: i64,
    limit: usize,
    cursor: Option<i64>,
) -> Result<WorkingSetPage> {
    let config = *config;
    let limit = limit.clamp(1, 4096);
    vault.read(vault_store::Db::Archive, move |s| {
        let mut page=s.query(
            "SELECT p.id,p.canonical_url,p.url,COALESCE(p.registrable_domain,''),
                (SELECT d.title FROM pages variant JOIN search_documents d ON d.page_id=variant.id
                 WHERE variant.canonical_url=p.canonical_url ORDER BY d.newest_visit_ms DESC,d.page_id DESC LIMIT 1),
                COALESCE((SELECT SUM(f.visible_visit_count) FROM pages variant JOIN search_document_profiles f ON f.page_id=variant.id WHERE variant.canonical_url=p.canonical_url LIMIT 1 OFFSET 0),0),
                COALESCE((SELECT MAX(f.newest_visit_ms) FROM pages variant JOIN search_document_profiles f ON f.page_id=variant.id WHERE variant.canonical_url=p.canonical_url LIMIT 1 OFFSET 0),0),
                EXISTS(SELECT 1 FROM star WHERE entity_kind='url' AND entity_key=p.canonical_url)
                 OR EXISTS(SELECT 1 FROM star WHERE entity_kind='domain' AND entity_key=p.registrable_domain),
                EXISTS(SELECT 1 FROM url_annotations WHERE canonical_url=p.canonical_url AND TRIM(notes)<>'')
                 OR EXISTS(SELECT 1 FROM url_tags WHERE canonical_url=p.canonical_url)
             FROM pages p WHERE p.id>?1
               AND p.id=(SELECT MIN(variant.id) FROM pages variant WHERE variant.canonical_url=p.canonical_url LIMIT 1 OFFSET 0)
             ORDER BY p.id LIMIT ?2",
            vault_store::params![cursor.unwrap_or(0),limit as i64+1],|r| {
                let signals=CandidateSignals {visit_count:r.get::<i64>(5)? as u32,
                    recent:r.get::<i64>(6)?>=recency_cutoff_ms(now_ms,config.recency_window_months),
                    starred:r.get::<i64>(7)?!=0,annotated:r.get::<i64>(8)?!=0};
                Ok((r.get::<i64>(0)?,WorkingSetCandidate { canonical_url:r.get(1)?,raw_url:Some(r.get(2)?),
                    domain:r.get(3)?,title:r.get(4)?,score:score_candidate(&signals,&config),signals }))
            })?;
        let next_cursor=if page.len()>limit {page.pop();Some(page.last().unwrap().0)} else {None};
        let mut items:Vec<_>=page.into_iter().map(|(_,item)|item).filter(|row|row.score>0.0 && (row.signals.visit_count>0 || row.signals.starred || row.signals.annotated)).collect();
        items.sort_by(|a,b|b.score.total_cmp(&a.score).then_with(||a.canonical_url.cmp(&b.canonical_url)));
        Ok(WorkingSetPage {items,next_cursor})
    })
}

/// Counts what a re-embed run for one scope would index (W-AI-9 Sub-block D, 05 §7 PME): never loads
/// a model, opens a network socket, or embeds anything, so it is cheap to call before every re-embed.
/// - [`ReembedScope::WorkingSet`] counts every eligible canonical page through `select_working_set`
///   continuation pages.
/// - [`ReembedScope::Full`] / [`ReembedScope::Incremental`] → `page_count` is the unique-page count, read
///   as `COUNT(*)` over the native `pages` table, never over the visit table. Incremental's true count is
///   "new/changed only"; the upper bound shown is the full unique-page count, so it never under-promises.
pub fn estimate_reembed(
    vault: &crate::Vault,
    config: &WorkingSetConfig,
    now_ms: i64,
    scope: ReembedScope,
) -> Result<ReembedEstimate> {
    let page_count = match scope {
        ReembedScope::WorkingSet => {
            let mut count = 0;
            let mut cursor = None;
            loop {
                let page = select_working_set(vault, config, now_ms, 4096, cursor)?;
                count += page.items.len() as u64;
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            count
        }
        ReembedScope::Full | ReembedScope::Incremental => unique_page_count(vault)?,
    };
    Ok(ReembedEstimate { scope, page_count })
}

/// Counts unique pages as `COUNT(*)` over `urls` (one row per unique URL — a single indexed count).
///
/// Bounded: never scans `visits`; the count is the upper bound on pages a full re-embed would touch.
fn unique_page_count(vault: &crate::Vault) -> Result<u64> {
    vault.read(vault_store::Db::Archive, |s| {
        Ok(s.query_one("SELECT COUNT(*) FROM pages", (), |r| Ok(r.get::<i64>(0)?))? as u64)
    })
}

/// Mutable per-canonical accumulator while signals from the four queries are merged.
#[cfg(test)]
#[derive(Debug, Default)]
struct CandidateState {
    signals: CandidateSignals,
    title: Option<String>,
}

#[cfg(test)]
impl CandidateState {
    /// Records the freshest non-empty title seen for this canonical URL (first writer wins, and the
    /// recency query feeds rows newest-first, so the first title is the freshest).
    fn note_title(&mut self, title: Option<String>) {
        if self.title.is_none() {
            if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
                self.title = Some(title);
            }
        }
    }

    /// Lifts the candidate's recorded visit count to the MAX across this page's raw variants.
    ///
    /// One page can surface under several raw URLs (tracking-param variants), each carrying its own
    /// `urls.visit_count`; the refind signal is the highest count among them. Negative counts (never
    /// expected from the schema) are clamped to 0. Pure → unit-tested so the max-tracking branch is
    /// covered without depending on a particular SQL row ordering.
    fn note_visit_count(&mut self, visit_count: i64) {
        let count = visit_count.max(0) as u32;
        if count > self.signals.visit_count {
            self.signals.visit_count = count;
        }
    }
}

/// Converts a recency window in months into the Chrome-epoch cutoff before which a visit is "old".
///
/// PURE so the window math is unit-tested. Uses a fixed 30-day month (the working set is a coarse
/// recency bucket, not a calendar computation) and saturates at 0 so an enormous window never
/// underflows. `now_ms` is Chrome-epoch microseconds-based ms as stored in `last_visit_ms`.
fn recency_cutoff_ms(now_ms: i64, window_months: u32) -> i64 {
    const MS_PER_DAY: i64 = 86_400_000;
    let window_ms = (window_months as i64).saturating_mul(30).saturating_mul(MS_PER_DAY);
    // Floor at 0: `last_visit_ms` is never before the epoch, so a window larger than `now` means
    // "everything is recent" — clamp to 0 rather than returning a negative cutoff.
    now_ms.saturating_sub(window_ms).max(0)
}

#[cfg(test)]
fn apply_starred_domains(
    starred_domains: &[String],
    by_canonical: &mut HashMap<String, CandidateState>,
) {
    use std::collections::HashSet;
    let domains: HashSet<&str> = starred_domains.iter().map(String::as_str).collect();
    for (canonical_url, state) in by_canonical.iter_mut() {
        let domain = registrable_domain_for_url(canonical_url).unwrap_or_default();
        if domains.contains(domain.as_str()) {
            state.signals.starred = true;
        }
    }
}

/// Canonicalizes a raw URL the SAME way stars/annotations/refind do (the dedup key, 05 §1).
///
/// Returns `None` for an unparseable URL so a single garbage row is skipped rather than failing the
/// whole gather. Centralized so every signal collapses tracking-param + host-casing variants of one
/// page onto the same candidate.
#[cfg(test)]
fn canonicalize(raw_url: &str) -> Option<String> {
    normalize_visit_url(raw_url).map(|normalized| normalized.canonical_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> WorkingSetConfig {
        WorkingSetConfig::default()
    }

    #[test]
    fn reembed_scope_defaults_to_incremental() {
        // The `#[serde(default)]` default must be Incremental so a frozen payload keeps prior behavior.
        assert_eq!(ReembedScope::default(), ReembedScope::Incremental);
    }

    #[test]
    fn default_config_puts_starred_on_top() {
        let config = config();
        // Starred outranks every other single signal (declared-signal-first, 05 §4).
        assert!(config.starred_weight > config.recent_weight);
        assert!(config.starred_weight > config.annotated_weight);
        assert!(config.starred_weight > config.frequency_weight);
    }

    #[test]
    fn score_candidate_sums_active_signals() {
        let config = config();
        // No signal → 0 (not a member).
        assert_eq!(score_candidate(&CandidateSignals::default(), &config), 0.0);

        // Only recent.
        let recent = CandidateSignals { recent: true, ..CandidateSignals::default() };
        assert_eq!(score_candidate(&recent, &config), config.recent_weight);

        // Only annotated.
        let annotated = CandidateSignals { annotated: true, ..CandidateSignals::default() };
        assert_eq!(score_candidate(&annotated, &config), config.annotated_weight);

        // Starred + recent sums both.
        let both = CandidateSignals { starred: true, recent: true, ..CandidateSignals::default() };
        assert_eq!(score_candidate(&both, &config), config.starred_weight + config.recent_weight);
    }

    #[test]
    fn score_candidate_log_damps_frequency() {
        let config = config();
        let one = CandidateSignals { visit_count: 1, ..CandidateSignals::default() };
        let many = CandidateSignals { visit_count: 5000, ..CandidateSignals::default() };
        let one_score = score_candidate(&one, &config);
        let many_score = score_candidate(&many, &config);
        // More visits ranks higher.
        assert!(many_score > one_score);
        // But log-damped: 5000 visits does NOT outrank a single STARRED page (declared signal wins).
        let starred = CandidateSignals { starred: true, ..CandidateSignals::default() };
        assert!(score_candidate(&starred, &config) > many_score);
        // visit_count 0 contributes nothing.
        assert_eq!(score_candidate(&CandidateSignals::default(), &config), 0.0);
    }

    #[test]
    fn recency_cutoff_subtracts_window_and_saturates() {
        // 1 month back from a large now.
        let now = 1_000_000_000_000_i64;
        let cutoff = recency_cutoff_ms(now, 1);
        assert_eq!(cutoff, now - 30 * 86_400_000);
        // A huge window saturates at 0 rather than underflowing.
        assert_eq!(recency_cutoff_ms(100, u32::MAX), 0);
        // Zero window = now itself.
        assert_eq!(recency_cutoff_ms(now, 0), now);
    }

    #[test]
    fn apply_starred_domains_marks_matching_candidates() {
        let mut map: HashMap<String, CandidateState> = HashMap::new();
        map.insert("https://example.com/a".to_string(), CandidateState::default());
        map.insert("https://other.com/b".to_string(), CandidateState::default());
        apply_starred_domains(&["example.com".to_string()], &mut map);
        assert!(map["https://example.com/a"].signals.starred);
        assert!(!map["https://other.com/b"].signals.starred);
    }

    #[test]
    fn candidate_state_tracks_max_visit_count_across_variants() {
        let mut state = CandidateState::default();
        state.note_visit_count(5);
        assert_eq!(state.signals.visit_count, 5);
        // A higher count from another raw variant lifts it.
        state.note_visit_count(42);
        assert_eq!(state.signals.visit_count, 42);
        // A lower count does NOT lower it (max-across-variants).
        state.note_visit_count(3);
        assert_eq!(state.signals.visit_count, 42);
        // A negative count clamps to 0 (never lowers below the recorded max).
        state.note_visit_count(-1);
        assert_eq!(state.signals.visit_count, 42);
    }

    #[test]
    fn candidate_state_keeps_first_nonempty_title() {
        let mut state = CandidateState::default();
        state.note_title(Some("  ".to_string())); // whitespace ignored
        assert_eq!(state.title, None);
        state.note_title(Some("First".to_string()));
        state.note_title(Some("Second".to_string())); // first writer wins
        assert_eq!(state.title.as_deref(), Some("First"));
    }

    #[test]
    fn canonicalize_collapses_variants_and_drops_garbage() {
        let a = canonicalize("https://Example.com/page?utm_source=x").expect("a");
        let b = canonicalize("https://example.com/page").expect("b");
        assert_eq!(a, b, "tracking params + host casing collapse onto one canonical URL");
        assert!(canonicalize("not a url").is_none());
    }
}
