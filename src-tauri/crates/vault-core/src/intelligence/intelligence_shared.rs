//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Shared Core Intelligence utility helpers.
//!
//! ## Responsibilities
//! - Convert between local-day keys, RFC3339 timestamps, and date-range bounds
//!   for deterministic read-model queries.
//! - Centralize query-token and navigational-noise heuristics so schema
//!   bootstrap, rebuilds, and read models do not drift.
//! - Keep small derived metrics such as KPI trend math and refind counts out of
//!   route-specific modules.
//!
//! ## Not responsible for
//! - Running rebuild stages or persisting deterministic rows.
//! - Loading archive visits or applying the site dictionary.
//! - Shaping route-level `/intelligence` payloads.
//!
//! ## Dependencies
//! - Parent-module `SearchQueryKind` and `DateRange` contracts.
//! - `reqwest::Url` for URL-like query detection.
//! - SQLite read access only for bounded aggregate helpers.
//!
//! ## Performance notes
//! - Tokenization and date helpers are pure, allocation-bounded utilities.
//! - `count_refind_pages_in_range` stays on indexed range filters instead of
//!   materializing refind rows in memory.

use super::SearchQueryKind;
use crate::models::DateRange;
use crate::visit_taxonomy::is_cjk_like;
use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use icu_segmenter::{WordSegmenter, WordSegmenterBorrowed, options::WordBreakInvariantOptions};
use reqwest::Url;
use std::collections::HashSet;
use std::sync::LazyLock;
use vault_store::{Session, params_owned as params};

/// Converts an archive timestamp into a UTC RFC3339 string for payloads that expose exact event
/// time.
pub(super) fn rfc3339_from_millis(visit_time_ms: i64) -> String {
    Utc.timestamp_millis_opt(visit_time_ms).single().unwrap_or_default().to_rfc3339()
}

/// In tests, a visit's day in the zone a first build takes (the computer's).
#[cfg(test)]
pub(super) fn local_date_key(visit_time_ms: i64) -> String {
    super::InsightsZone::system().date_key(visit_time_ms)
}

/// Expands a user-facing inclusive date range into `[start_ms, end_ms)` bounds, in the zone the
/// published Insights put their days in (`InsightsZone::for_reading`).
pub(super) fn date_range_bounds(s: &Session<'_>, range: &DateRange) -> Result<(i64, i64)> {
    super::InsightsZone::for_reading(s)?.range_bounds(range)
}

/// Word breaks for Chinese and Japanese, which put no spaces between words: ICU4X's dictionary
/// for them (and its small models for Thai, Lao, Khmer and Burmese). Built once.
static WORD_SEGMENTER: LazyLock<WordSegmenterBorrowed<'static>> =
    LazyLock::new(|| WordSegmenter::new_auto(WordBreakInvariantOptions::default()));

/// A query's tokens before function words are taken out: lower case, split at anything that is
/// neither a letter nor a digit. A token can hold a whole Chinese or Japanese phrase.
fn raw_tokens(query: &str) -> impl Iterator<Item = String> + '_ {
    query
        .split(|ch: char| !ch.is_alphanumeric() && !is_cjk_like(ch))
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
}

/// A phrase's dictionary words, each with whether it is a word at all (not a space or a mark).
fn cjk_words(token: &str) -> Vec<(&str, bool)> {
    let mut words = Vec::new();
    let mut breaks = WORD_SEGMENTER.segment_str(token);
    let mut start = breaks.next().unwrap_or(0);
    while let Some(end) = breaks.next() {
        words.push((&token[start..end], breaks.is_word_like()));
        start = end;
    }
    words
}

/// The words a search is shown and counted under ("What you search for"): Latin text split at
/// anything that is not a letter or digit, Chinese and Japanese at their dictionary word breaks
/// (「台北天氣」 is 台北 / 天氣), function words and URL parts left out.
pub(super) fn tokenize_query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for token in raw_tokens(query) {
        if token.chars().any(is_cjk_like) {
            terms.extend(
                cjk_words(&token)
                    .into_iter()
                    .filter(|(word, is_word)| *is_word && !is_stop_word(word))
                    .map(|(word, _)| word.to_owned()),
            );
        } else if !is_stop_word(&token) {
            terms.push(token);
        }
    }
    terms
}

/// Function words (English, Chinese in both scripts, Japanese particles and endings) and URL
/// parts. They join unrelated searches: with "this" a term, a search for "this" counted as a
/// rewording of every search containing it (31 times on a real archive), and "this" ranked
/// among the words searched most; 什麼, 如何 and の do the same once Chinese and Japanese
/// searches are split into words.
fn is_stop_word(token: &str) -> bool {
    const CJK: &str = "
        的 了 是 在 和 與 与 及 或 嗎 吗 呢 吧 啊 呀 嘛 喔 哦 很 也 都 就 還 还 又 再 被 把 給 给 對 对
        從 从 到 跟 為 为 以 之 其 等 有 沒 没 不 會 会 能 要 可 可以 一個 一个 個 个 這 这 那 這個 这个
        那個 那个 這些 这些 那些 什麼 什么 甚麼 怎麼 怎么 怎樣 怎样 如何 為什麼 为什么 為何 为何 哪 哪裡
        哪里 哪個 哪个 誰 谁 幾 几 多少 我 你 妳 您 他 她 它 我們 我们 你們 你们 他們 他们 自己
        の は が を に で と も へ や か な ね よ から まで より です ます でした ました だ する した して
        ある いる こと もの これ それ あれ この その あの どう どの なに 何 ない
    ";
    const LATIN: &str = "
        a about above after again against all am an and any are as at be because been before
        being below between both but by can com could did do does doing down during each edu few
        for from further had has have having he her here hers herself him himself his how html
        http https i if in into is it its itself just me more most my myself net no nor not of
        off on once only or org other our ours ourselves out over own same she should so some
        such than that the their theirs them themselves then there these they this those through
        to too under until up versus very vs was we were what when where which while who whom
        why will with would www you your yours yourself yourselves
    ";
    static STOP_WORDS: LazyLock<HashSet<&str>> =
        LazyLock::new(|| CJK.split_whitespace().chain(LATIN.split_whitespace()).collect());
    STOP_WORDS.contains(token)
}

/// The words a keyword search is counted under in "Search concepts": its query terms without
/// single Latin letters or digits and bare numbers (the "s" of "asu's", the "3" of "python 3"),
/// which say nothing on their own. Han, kana and Hangul words count at any length.
pub(super) fn search_words(query: &str) -> std::collections::BTreeSet<String> {
    tokenize_query_terms(query)
        .into_iter()
        .filter(|term| {
            term.chars().any(is_cjk_like)
                || (term.chars().count() > 1 && !term.chars().all(|ch| ch.is_ascii_digit()))
        })
        .collect()
}

/// The set two searches (or a page title and a search) are compared by, for rewordings, query
/// families and trails: Latin words as in [`tokenize_query_terms`]; for Chinese and Japanese,
/// what is left of each phrase once its function words are out, in overlapping pairs of
/// characters. A dictionary splits the same phrase differently by what surrounds it (什麼是非同步
/// gives 是非 / 同步, 非同步是什麼 gives 非同 / 步), but the pairs 非同 and 同步 come out of both.
pub(super) fn query_token_set(query: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for token in raw_tokens(query) {
        if !token.chars().any(is_cjk_like) {
            if !is_stop_word(&token) {
                set.insert(token);
            }
            continue;
        }
        let mut phrase = String::new();
        for (word, is_word) in cjk_words(&token) {
            if !is_word || is_stop_word(word) {
                insert_pairs(&mut phrase, &mut set);
            } else if word.chars().any(is_cjk_like) {
                phrase.push_str(word);
            } else {
                insert_pairs(&mut phrase, &mut set);
                set.insert(word.to_owned());
            }
        }
        insert_pairs(&mut phrase, &mut set);
    }
    set
}

/// Adds a phrase's overlapping pairs of characters (the character itself when it stands alone)
/// and empties it.
fn insert_pairs(phrase: &mut String, set: &mut HashSet<String>) {
    let chars = phrase.chars().collect::<Vec<_>>();
    match chars.len() {
        0 => {}
        1 => {
            set.insert(chars[0].to_string());
        }
        _ => set.extend(chars.windows(2).map(|pair| pair.iter().collect::<String>())),
    }
    phrase.clear();
}

/// Distinguishes keyword queries from navigational noise without requiring any
/// LLM interpretation or cross-surface special cases.
pub(super) fn classify_search_query_kind(
    raw_query: &str,
    normalized_query: &str,
    landing_domain: Option<&str>,
) -> SearchQueryKind {
    if normalized_query.trim().is_empty() {
        return SearchQueryKind::Navigational;
    }

    if let Some(candidate_domain) =
        query_domain_candidate(raw_query).or_else(|| query_domain_candidate(normalized_query))
    {
        let landing_matches = landing_domain.is_none_or(|domain| {
            domain == candidate_domain
                || landing_domain_matches_candidate(domain, &candidate_domain)
        });
        if landing_matches {
            return SearchQueryKind::Navigational;
        }
    }

    SearchQueryKind::Keyword
}

/// Computes set overlap for the deterministic query-family heuristic.
pub(super) fn jaccard(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    intersection / union
}

/// Shifts a date range backwards by its own inclusive span so KPI comparisons
/// use the immediately preceding window.
pub(super) fn previous_date_range(range: &DateRange) -> Result<DateRange> {
    let start = NaiveDate::parse_from_str(&range.start, "%Y-%m-%d")?;
    let end = NaiveDate::parse_from_str(&range.end, "%Y-%m-%d")?;
    let days = (end - start).num_days().max(0) + 1;
    let prev_end = start - Duration::days(1);
    let prev_start = prev_end - Duration::days(days - 1);
    Ok(DateRange {
        start: prev_start.format("%Y-%m-%d").to_string(),
        end: prev_end.format("%Y-%m-%d").to_string(),
    })
}

/// "Pages reopened": the refind pages opened on a day of the range, the same pages the day page
/// lists, counted once across browsers when no profile is chosen (`refind_days`). A page's
/// first-to-last span overlapping the range is not enough: a day with no visits showed every
/// long-lived page.
pub(super) fn count_refind_pages_in_range(
    connection: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    super::refind_days::count(connection, range, profile_id)
}

/// Builds the canonical KPI delta object shared by summary surfaces and
/// regression assertions.
pub(super) fn build_kpi(current: i64, previous: i64) -> crate::models::KpiMetric {
    let trend = if current > previous {
        "up"
    } else if current < previous {
        "down"
    } else {
        "flat"
    };
    let change_percent = if previous == 0 {
        None
    } else {
        Some(((current - previous) as f32 / previous as f32) * 100.0)
    };
    crate::models::KpiMetric {
        value: current,
        previous_value: Some(previous),
        change_percent,
        trend: trend.to_string(),
    }
}

/// Collapses exact day keys into the user-requested reporting granularity while
/// preserving existing week/month/year bucket grammar.
pub(super) fn collapse_date_key(date_key: &str, granularity: &str) -> String {
    match granularity {
        "week" => {
            if let Ok(date) = NaiveDate::parse_from_str(date_key, "%Y-%m-%d") {
                let iso = date.iso_week();
                format!("{}-W{:02}", iso.year(), iso.week())
            } else {
                date_key.to_string()
            }
        }
        "month" => date_key.get(0..7).unwrap_or(date_key).to_string(),
        "year" => date_key.get(0..4).unwrap_or(date_key).to_string(),
        _ => date_key.to_string(),
    }
}

fn landing_domain_matches_candidate(landing_domain: &str, candidate_domain: &str) -> bool {
    landing_domain == candidate_domain
        || landing_domain.ends_with(&format!(".{candidate_domain}"))
        || candidate_domain.ends_with(&format!(".{landing_domain}"))
}

fn query_domain_candidate(query: &str) -> Option<String> {
    let trimmed = query.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return None;
    }

    let parsed = Url::parse(trimmed).ok().or_else(|| {
        if trimmed.contains('.') { Url::parse(&format!("https://{trimmed}")).ok() } else { None }
    })?;
    let host = parsed.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    Some(crate::visit_taxonomy::registrable_domain_for_host(&host))
}

/// How many sites were new on each day of `range`. A site is new on the first day it appears
/// in any browser profile, or in `profile_id` when one is given. Each profile's own rollups mark
/// its first day for a site (`new_domain_visits`), so the whole archive takes the earliest of
/// those; adding them up instead would count a site once per profile that visited it.
pub(crate) fn new_sites_by_day(
    connection: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<std::collections::BTreeMap<String, i64>> {
    let rows = connection.query(
        "SELECT first_day, COUNT(*) FROM (
           SELECT MIN(r.date_key) AS first_day
           FROM domain_daily_rollups_rows r INDEXED BY idx_domain_daily_first_day
           WHERE r.new_domain_visits > 0
             AND r.generation IN (SELECT daily FROM intelligence_generations)
             AND (?1 IS NULL OR r.profile_id = ?1)
           GROUP BY r.registrable_domain)
         WHERE first_day >= ?2 AND first_day <= ?3
         GROUP BY first_day",
        params![profile_id, range.start.as_str(), range.end.as_str()],
        |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?)),
    )?;
    Ok(rows.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chinese and Japanese put no spaces between words. Split only at spaces, a search was one
    /// word, so a rewording of it shared nothing with it and Search concepts listed whole
    /// sentences.
    ///
    /// Ways it can go wrong, each checked: the phrase stays one word; its function words survive
    /// and join unrelated searches; the same words in another order no longer compare as one
    /// search (a dictionary splits them differently by context); Simplified and Traditional
    /// come apart; a Latin part of a mixed phrase is lost; Latin text splits differently.
    #[test]
    fn chinese_and_japanese_searches_split_into_words() {
        let set =
            |terms: &[&str]| terms.iter().map(|term| term.to_string()).collect::<HashSet<_>>();
        let shown = |query: &str| tokenize_query_terms(query);
        let compared = |query: &str| query_token_set(query);
        // A rewording, as trails and families decide it: one set holds the other, or they share
        // at least half their tokens.
        let one_search = |a: &HashSet<String>, b: &HashSet<String>| {
            let shared = a.intersection(b).count();
            shared == a.len() || shared == b.len() || shared * 2 >= a.union(b).count()
        };

        assert_eq!(shown("tokio vs smol"), ["tokio", "smol"]);
        assert_eq!(compared("tokio vs smol"), set(&["tokio", "smol"]));

        let (asked, turned) = (compared("什麼是非同步"), compared("非同步是什麼"));
        for tokens in [&asked, &turned] {
            assert!(!tokens.is_empty());
            for whole_or_function in ["什麼是非同步", "非同步是什麼", "什麼", "是"] {
                assert!(!tokens.contains(whole_or_function), "{tokens:?}");
            }
        }
        assert!(
            one_search(&asked, &turned),
            "the same words in another order: {asked:?} {turned:?}"
        );
        let words = shown("什麼是非同步");
        assert!(!words.is_empty() && !words.iter().any(|word| word == "什麼是非同步"), "{words:?}");

        for (query, word) in [("如何安裝rust", "安裝"), ("如何安装rust", "安装")] {
            let words = shown(query);
            assert!(
                words.iter().any(|w| w == word) && words.iter().any(|w| w == "rust"),
                "{query}: {words:?}"
            );
            assert!(!words.iter().any(|w| w == "如何"), "{query}: {words:?}");
            assert!(compared(query).contains("rust"), "{query}");
        }
        assert_eq!(shown("東京の天気"), ["東京", "天気"]);
        assert!(shown("什麼是").is_empty() && compared("什麼是").is_empty(), "only function words");

        // The E2E fixture's searches (`FIXTURE_CJK_WORDS`): shown as these words, and one search.
        assert_eq!(shown("台北天氣"), ["台北", "天氣"]);
        assert_eq!(shown("台北天氣預報"), ["台北", "天氣預報"]);
        assert!(one_search(&compared("台北天氣"), &compared("台北天氣預報")));
    }
}
