//! URL normalization and registrable-domain helpers.
//!
//! ## Responsibilities
//! - Normalize visit URLs into deterministic matching records.
//! - Strip tracking parameters while preserving semantic query parameters.
//! - Derive registrable domains and subdomains for taxonomy matching.
//!
//! ## Not responsible for
//! - Applying taxonomy rule packs or user overrides.
//! - Loading browser rows from canonical archive tables.
//! - Persisting canonical URLs.
//!
//! ## Dependencies
//! - `reqwest::Url` for URL parsing and canonical query serialization.
//! - `publicsuffix` plus a small fallback list for common multi-label suffixes.
//!
//! ## Performance notes
//! - Work is bounded to one URL at a time. Callers processing millions of rows
//!   should stream visits instead of materializing every normalized URL.

use super::{text::normalize_whitespace, types::NormalizedVisitUrl};
use publicsuffix::{IcannList, Psl};
use reqwest::Url;
use std::sync::LazyLock;

const SEARCH_QUERY_KEYS: &[&str] =
    &["q", "query", "query_text", "search_query", "p", "wd", "word", "text", "keyword", "k"];
const COMMON_MULTI_LABEL_PUBLIC_SUFFIXES: &[&str] = &[
    "co.jp", "co.kr", "co.uk", "com.au", "com.cn", "com.hk", "com.sg", "com.tr", "com.tw",
    "net.cn", "org.cn",
];
static PUBLIC_SUFFIX_LIST: LazyLock<IcannList> = LazyLock::new(IcannList::default);

/// Normalizes a raw visit URL into a deterministic matching form.
pub fn normalize_visit_url(raw_url: &str) -> Option<NormalizedVisitUrl> {
    let mut parsed = Url::parse(raw_url).ok()?;
    let host = parsed.host_str()?.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    let mut preserved_query = Vec::new();
    let mut dropped_tracking_params = Vec::new();
    let mut search_query = None;
    for (key, value) in parsed.query_pairs() {
        let key = key.to_string();
        let value = normalize_whitespace(&value.replace('+', " "));
        if is_tracking_param(&key) {
            dropped_tracking_params.push(key);
            continue;
        }
        if search_query.is_none()
            && is_search_engine_host(&host)
            && SEARCH_QUERY_KEYS.iter().any(|candidate| candidate.eq_ignore_ascii_case(&key))
            && !value.is_empty()
        {
            search_query = Some(value.clone());
        }
        preserved_query.push((key, value));
    }

    parsed.set_query(None);
    if !preserved_query.is_empty() {
        let mut serializer = parsed.query_pairs_mut();
        for (key, value) in &preserved_query {
            serializer.append_pair(key, value);
        }
    }

    let registrable_domain = registrable_domain_for_host(&host);
    let subdomain = subdomain_for_host_and_domain(&host, &registrable_domain);
    Some(NormalizedVisitUrl {
        canonical_url: parsed.to_string(),
        host: host.clone(),
        registrable_domain,
        subdomain,
        path: parsed.path().to_string(),
        preserved_query,
        dropped_tracking_params,
        search_query: search_query.clone(),
        is_search_results: search_query.is_some() && is_search_engine_host(&host),
    })
}

/// Extracts a search query term from a URL when one is present.
pub fn extract_search_query_from_url(url: &str) -> Option<String> {
    normalize_visit_url(url).and_then(|value| value.search_query)
}

/// Returns the registrable domain for a full URL when one can be derived.
pub fn registrable_domain_for_url(url: &str) -> Option<String> {
    normalize_visit_url(url).map(|value| value.registrable_domain)
}

/// Returns the registrable domain for a host, honoring common multi-label suffixes.
pub fn registrable_domain_for_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    let fallback = fallback_registrable_domain_for_host(&host);
    match (PUBLIC_SUFFIX_LIST.domain(host.as_bytes()), PUBLIC_SUFFIX_LIST.suffix(host.as_bytes())) {
        (Some(domain), Some(suffix)) if domain.as_bytes() != suffix.as_bytes() => {
            let candidate = String::from_utf8_lossy(domain.as_bytes()).to_string();
            if candidate.split('.').count() < fallback.split('.').count() {
                fallback
            } else {
                candidate
            }
        }
        _ => fallback,
    }
}

/// Matches either an exact host or a subdomain of the provided suffix.
pub(super) fn host_matches_suffix(host: &str, suffix: &str) -> bool {
    host == suffix || host.ends_with(&format!(".{suffix}"))
}

/// Provides a deterministic fallback when PSL data is too broad for known hosts.
fn fallback_registrable_domain_for_host(host: &str) -> String {
    let segments = host.split('.').collect::<Vec<_>>();
    if segments.len() <= 2 {
        host.to_string()
    } else if segments.len() >= 3 {
        let suffix = format!("{}.{}", segments[segments.len() - 2], segments[segments.len() - 1]);
        if COMMON_MULTI_LABEL_PUBLIC_SUFFIXES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&suffix))
        {
            return segments[segments.len() - 3..].join(".");
        }
        segments[segments.len() - 2..].join(".")
    } else {
        segments[segments.len() - 2..].join(".")
    }
}

/// Detects built-in search-engine hosts that can safely expose query terms.
fn is_search_engine_host(host: &str) -> bool {
    let domain = registrable_domain_for_host(host);
    matches!(
        domain.as_str(),
        "baidu.com"
            | "bing.com"
            | "brave.com"
            | "duckduckgo.com"
            | "google.com"
            | "sogou.com"
            | "so.com"
            | "yahoo.com"
            | "yandex.ru"
    ) || host.starts_with("www.google.")
        || host == "search.brave.com"
}

/// Detects query parameters that should not affect canonical visit identity.
fn is_tracking_param(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_")
        || matches!(
            key.as_str(),
            "fbclid"
                | "gclid"
                | "igshid"
                | "mc_cid"
                | "mc_eid"
                | "mkt_tok"
                | "ref"
                | "ref_src"
                | "si"
                | "spm"
                | "source"
                | "sourceid"
        )
}

/// Returns the host prefix that remains after removing the registrable domain.
fn subdomain_for_host_and_domain(host: &str, registrable_domain: &str) -> Option<String> {
    if host == registrable_domain {
        return None;
    }
    let suffix = format!(".{registrable_domain}");
    host.strip_suffix(&suffix).map(|value| value.to_string()).filter(|value| !value.is_empty())
}
