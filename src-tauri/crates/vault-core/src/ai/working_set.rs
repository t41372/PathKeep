//! Heavy-working-set priority selector — the shared hook for the heavy tier + enrichment (W-AI-4c).
//!
//! ## Responsibilities
//! - rank/select UNIQUE-CONTENT candidates (one entry per canonical URL — the dedup unit, 05 §1) for
//!   the HEAVY embedding tier (Qwen3, W-AI-4d) and the enrichment fetch queue (W-ENRICH-1) by the
//!   declared working-set signals (05 §4/§8): **starred (top weight) ∪ recent (configurable window)
//!   ∪ tagged/noted (annotations) ∪ high-frequency (refind / visit-count)**.
//! - keep the candidate gather BOUNDED + INDEXED (no full 14.4M in-memory scan): each signal feeds a
//!   bounded query off an existing index, the union is capped, and the final ranking is a small
//!   in-memory sort over the (bounded) candidate set.
//!
//! ## Not responsible for
//! - actually embedding the heavy tier (W-AI-4d) or fetching enrichment (W-ENRICH-1) — this only
//!   SELECTS + RANKS; the consumers run the work. The STATIC base tier embeds 100% and does NOT need
//!   this selector (05 §4) — this is purely the heavy/enrichment prioritization hook.
//! - canonical URL identity (it reuses `visit_taxonomy::normalize_visit_url`, the same
//!   canonicalization stars/annotations/refind use) or the `star` schema.
//!
//! ## Why this module exists
//! 05 §4/§8 + 06 §5 both want the SAME prioritized candidate list ("a small set of URLs carries most
//! queries / most enrichment value"). Putting it in ONE queryable, bounded, indexed selector means
//! W-AI-4d (heavy tier) and W-ENRICH-1 (content fetch) consume one ranking rather than each
//! re-deriving the working set — and resolves the `// TODO(W-AI-4c/heavy-tier)` seams left in
//! `stars.rs` / `indexing.rs`.
//!
//! ## Performance notes
//! - The four signal queries each ride an existing index: `star` via `idx_star_kind_starred_at`,
//!   recency via `idx_urls_profile_last_visit` / `last_visit_ms`, high-frequency via `visit_count`,
//!   annotations via `url_annotations`/`idx_url_tags_tag`. None scans the whole `urls`/`visits` table
//!   unbounded; the per-signal `LIMIT` keeps each gather O(window), and the final ranking sorts only
//!   the (bounded) union — so the selector stays cheap at the 14.4M tail.

use crate::utils::url_domain;
use crate::visit_taxonomy::{normalize_visit_url, registrable_domain_for_url};
use anyhow::{Context, Result};
use rusqlite::Connection;
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

/// Upper bound on how many candidates one [`select_working_set`] call returns.
///
/// The heavy tier + enrichment queue are BOUNDED working sets (05 §4: never the whole corpus), so a
/// generous-but-finite cap keeps the selection — and everything downstream consumes from it —
/// bounded even on the 14.4M tail. A caller may pass a smaller limit; this is the hard ceiling.
pub const MAX_WORKING_SET: usize = 50_000;

/// Per-signal gather cap, so each bounded query contributes at most this many rows to the union.
///
/// Each of the four signals pulls its top rows off an index (recent by `last_visit_ms`, frequent by
/// `visit_count`, etc.); capping each keeps the union — and the in-memory ranking sort — bounded
/// regardless of archive size. Sized at a FRACTION of [`MAX_WORKING_SET`] (a quarter) so no single
/// signal can fill the union on its own (finding S6): even if recency alone could supply 50k rows, it
/// contributes at most this many, leaving room for the other declared signals (starred / annotated /
/// frequent) to genuinely blend into the working set rather than one signal dominating it. The four
/// signals together can still comfortably fill `MAX_WORKING_SET`.
const PER_SIGNAL_GATHER_CAP: usize = MAX_WORKING_SET / 4;

/// Selects + ranks the heavy-tier / enrichment working set from the canonical archive (bounded).
///
/// The shared hook (05 §4/§8, 06 §5): gathers candidates from the four declared signals — each via a
/// bounded, indexed query — canonicalizes every raw URL through the SAME `normalize_visit_url`
/// stars/annotations/refind use (so one canonical page is ONE candidate however it was reached, 05
/// §1), merges the signals per canonical URL, scores each, and returns the top `limit` by score
/// (descending; ties broken by canonical URL for determinism). `now_ms` is the current Chrome-epoch
/// time the recency window is measured against (passed in so the ranking is deterministic + testable).
///
/// The STATIC base tier does NOT call this (it embeds 100%); this is purely the heavy-tier /
/// enrichment prioritizer. Returns at most `min(limit, MAX_WORKING_SET)` candidates, all with a
/// non-zero score (a page with no active signal is never a working-set member).
pub fn select_working_set(
    connection: &Connection,
    config: &WorkingSetConfig,
    now_ms: i64,
    limit: usize,
) -> Result<Vec<WorkingSetCandidate>> {
    let cap = limit.min(MAX_WORKING_SET);
    if cap == 0 {
        return Ok(Vec::new());
    }

    // Per-canonical-URL accumulator: signals merged across however many raw URLs collapse onto it.
    let mut by_canonical: HashMap<String, CandidateState> = HashMap::new();

    let recency_cutoff_ms = recency_cutoff_ms(now_ms, config.recency_window_months);
    gather_url_signals(connection, recency_cutoff_ms, &mut by_canonical)?;
    gather_annotation_signals(connection, &mut by_canonical)?;
    gather_starred_signals(connection, &mut by_canonical)?;

    // Score + rank the (bounded) union. The sort is over the candidate set only — never the archive.
    let mut ranked: Vec<WorkingSetCandidate> = by_canonical
        .into_iter()
        .filter_map(|(canonical_url, state)| {
            let score = score_candidate(&state.signals, config);
            if score <= 0.0 {
                return None; // No active signal → not a working-set member.
            }
            Some(WorkingSetCandidate {
                domain: url_domain(&canonical_url),
                title: state.title,
                score,
                signals: state.signals,
                canonical_url,
            })
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.canonical_url.cmp(&b.canonical_url))
    });
    ranked.truncate(cap);
    Ok(ranked)
}

/// Mutable per-canonical accumulator while signals from the four queries are merged.
#[derive(Debug, Default)]
struct CandidateState {
    signals: CandidateSignals,
    title: Option<String>,
}

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

/// Gathers the recency + frequency signals (and titles) from `urls`, bounded by index-ordered limits.
///
/// Two bounded queries off `urls`: the most-recently-visited rows (recency signal + the recency
/// window check) and the highest-visit-count rows (frequency signal). Both are `LIMIT`-capped and
/// ride `last_visit_ms` / `visit_count` orderings, so neither scans the whole table. Each raw URL is
/// canonicalized so tracking-param variants of one page merge into a single candidate.
fn gather_url_signals(
    connection: &Connection,
    recency_cutoff_ms: i64,
    by_canonical: &mut HashMap<String, CandidateState>,
) -> Result<()> {
    // Recency: newest-visited first; mark `recent` only for rows inside the window.
    let mut recency = connection
        .prepare(
            "SELECT url, title, visit_count, last_visit_ms
             FROM urls
             ORDER BY last_visit_ms DESC
             LIMIT ?1",
        )
        .context("preparing working-set recency query")?;
    let mut rows = recency.query([PER_SIGNAL_GATHER_CAP as i64])?;
    while let Some(row) = rows.next()? {
        let raw_url: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        let last_visit_ms: i64 = row.get(3)?;
        let Some(canonical) = canonicalize(&raw_url) else { continue };
        let state = by_canonical.entry(canonical).or_default();
        state.note_title(title);
        if last_visit_ms >= recency_cutoff_ms {
            state.signals.recent = true;
        }
        state.note_visit_count(visit_count);
    }

    // Frequency: highest visit_count first, so a heavily-revisited page is captured even if it has
    // not been visited recently (it would fall outside the recency LIMIT window otherwise).
    let mut frequency = connection
        .prepare(
            "SELECT url, title, visit_count
             FROM urls
             WHERE visit_count > 0
             ORDER BY visit_count DESC
             LIMIT ?1",
        )
        .context("preparing working-set frequency query")?;
    let mut rows = frequency.query([PER_SIGNAL_GATHER_CAP as i64])?;
    while let Some(row) = rows.next()? {
        let raw_url: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        let Some(canonical) = canonicalize(&raw_url) else { continue };
        let state = by_canonical.entry(canonical).or_default();
        state.note_title(title);
        state.note_visit_count(visit_count);
    }
    Ok(())
}

/// Gathers the annotation signal: any URL with a note or a tag is a deliberate keep signal.
///
/// Annotations key by RAW url (05 §1 / migration 011), so each raw key is canonicalized to merge onto
/// the same candidate the URL signals built. Both reads are bounded and ride the annotation tables'
/// own (small) row counts — users annotate hundreds of pages, not millions.
fn gather_annotation_signals(
    connection: &Connection,
    by_canonical: &mut HashMap<String, CandidateState>,
) -> Result<()> {
    let mut mark = |raw_url: String| {
        if let Some(canonical) = canonicalize(&raw_url) {
            by_canonical.entry(canonical).or_default().signals.annotated = true;
        }
    };

    // Notes: a non-empty notes body marks the URL annotated.
    let mut notes = connection
        .prepare("SELECT url FROM url_annotations WHERE TRIM(notes) <> '' LIMIT ?1")
        .context("preparing working-set notes query")?;
    let mut rows = notes.query([PER_SIGNAL_GATHER_CAP as i64])?;
    while let Some(row) = rows.next()? {
        mark(row.get::<_, String>(0)?);
    }

    // Tags: any tag marks the URL annotated (DISTINCT so a multi-tag URL is counted once).
    let mut tags = connection
        .prepare("SELECT DISTINCT url FROM url_tags LIMIT ?1")
        .context("preparing working-set tags query")?;
    let mut rows = tags.query([PER_SIGNAL_GATHER_CAP as i64])?;
    while let Some(row) = rows.next()? {
        mark(row.get::<_, String>(0)?);
    }
    Ok(())
}

/// Gathers the starred signal (top weight) from the `star` table.
///
/// URL stars already key by `canonical_url` (stars canonicalize on write), so they merge directly.
/// Domain stars mark EVERY candidate on that registrable domain — the user starred the source, so
/// every page under it inherits the top signal. The starred set is tiny (users star hundreds), so
/// both reads ride `idx_star_kind_starred_at` and the domain match is an in-memory suffix check over
/// the (already bounded) candidate union rather than a `urls` scan.
fn gather_starred_signals(
    connection: &Connection,
    by_canonical: &mut HashMap<String, CandidateState>,
) -> Result<()> {
    let mut statement = connection
        .prepare("SELECT entity_kind, entity_key FROM star LIMIT ?1")
        .context("preparing working-set star query")?;
    let mut rows = statement.query([PER_SIGNAL_GATHER_CAP as i64])?;
    let mut starred_domains: Vec<String> = Vec::new();
    while let Some(row) = rows.next()? {
        let kind: String = row.get(0)?;
        let key: String = row.get(1)?;
        match kind.as_str() {
            "url" => {
                // A URL star's key is already canonical; mark it (creating the candidate if a star
                // exists for a page the archive has not surfaced via the other signals yet).
                by_canonical.entry(key).or_default().signals.starred = true;
            }
            "domain" => starred_domains.push(key),
            _ => {} // Unknown/future kinds (e.g. query_family) are ignored, not errored.
        }
    }
    // Domain stars: mark every candidate whose canonical URL is on a starred registrable domain.
    if !starred_domains.is_empty() {
        apply_starred_domains(&starred_domains, by_canonical);
    }
    Ok(())
}

/// Marks every candidate whose canonical URL sits on a starred registrable domain.
///
/// Split out so it is unit-testable directly with a synthetic candidate map (no SQL): for each
/// candidate, derive its REGISTRABLE domain (the same form a domain star keys by — `set_star`
/// canonicalizes a domain via `registrable_domain_for_url`) and mark it starred if that domain was
/// starred. Bounded by the candidate union size (already capped), so this is O(candidates ×
/// starred_domains) over small sets, never an archive scan.
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
    fn per_signal_cap_is_a_strict_fraction_of_max_so_no_signal_dominates() {
        // S6: a per-signal cap equal to MAX would let ONE signal fill the entire union, contradicting
        // the "no single signal can dominate" intent. It must be a strict fraction of MAX. These are
        // compile-time invariants over consts, so assert them in `const` blocks (clippy guidance).
        const {
            assert!(PER_SIGNAL_GATHER_CAP < MAX_WORKING_SET);
            assert!(PER_SIGNAL_GATHER_CAP == MAX_WORKING_SET / 4);
            // The four signals together still comfortably exceed MAX, so the union can fill it.
            assert!(PER_SIGNAL_GATHER_CAP * 4 >= MAX_WORKING_SET);
        }
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
