//! URL normalization and registrable-domain helpers.
//!
//! ## Responsibilities
//! - Normalize visit URLs into stable matching records.
//! - Strip tracking parameters while preserving semantic query parameters.
//! - Derive registrable domains and subdomains for taxonomy matching.
//!
//! ## Not responsible for
//! - Applying taxonomy rule packs or user overrides.
//! - Loading browser rows from canonical archive tables.
//! - Persisting canonical URLs.
//!
//! Failure modes: URL variants receive different canonical keys, semantic query values
//! are lost, or decoding an escaped delimiter changes the path. Tests: pages::tests
//! and visit_taxonomy::tests.
//!
//! ## Dependencies
//! - `reqwest::Url` for URL parsing and canonical query serialization.
//! - `publicsuffix`, reading Mozilla's Public Suffix List as vendored in `vendor/publicsuffix/`.
//!
//! ## Performance notes
//! - Work is bounded to one URL at a time. Callers processing millions of rows
//!   should stream visits instead of materializing every normalized URL.

use super::{text::normalize_whitespace, types::NormalizedVisitUrl};
use publicsuffix::{IcannList, List, Psl, Type};
use reqwest::Url;
use std::{net::IpAddr, sync::LazyLock};

const SEARCH_QUERY_KEYS: &[&str] =
    &["q", "query", "query_text", "search_query", "p", "wd", "word", "text", "keyword", "k"];
/// Mozilla's Public Suffix List, as `scripts/update-public-suffix-list.mjs` last fetched it.
const PUBLIC_SUFFIX_LIST: &[u8] =
    include_bytes!("../../vendor/publicsuffix/public_suffix_list.dat");

/// Both sections of the list, as browsers use it to tell sites apart: `rust-lang.github.io` and
/// `tokio-rs.github.io` are two sites, `news.bbc.co.uk` and `www.bbc.co.uk` one.
static SITES: LazyLock<List> =
    LazyLock::new(|| List::from_bytes(PUBLIC_SUFFIX_LIST).expect("the vendored suffix list"));

/// Its ICANN section alone: the domain a site's owner registered (`readthedocs.io` for every
/// `*.readthedocs.io` site, which the private section makes sites of their own).
static OWNERS: LazyLock<IcannList> =
    LazyLock::new(|| IcannList::from_bytes(PUBLIC_SUFFIX_LIST).expect("the vendored suffix list"));

/// The list's own version (its `// VERSION:` line). What a site is changes with it, so it is part
/// of the URL rules' version, and the archive records it for the pages it split.
pub(crate) fn public_suffix_list_version() -> &'static str {
    static VERSION: LazyLock<String> = LazyLock::new(|| {
        String::from_utf8_lossy(PUBLIC_SUFFIX_LIST)
            .lines()
            .find_map(|line| line.strip_prefix("// VERSION:"))
            .map(|version| version.trim().to_owned())
            .expect("the vendored suffix list names its version")
    });
    &VERSION
}

/// Normalizes a raw visit URL into a stable matching form.
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

    parsed.set_path(&canonical_path(parsed.path()));
    parsed.set_query(None);
    if !preserved_query.is_empty() {
        let mut serializer = parsed.query_pairs_mut();
        for (key, value) in &preserved_query {
            serializer.append_pair(key, value);
        }
    }

    // Only a web page belongs to a site. An extension's page (`chrome-extension://<id>/…`) keeps
    // its URL but has no site: its "host" is an id, not somewhere anyone browses to.
    let registrable_domain = if matches!(parsed.scheme(), "http" | "https") {
        registrable_domain_for_host(&host)
    } else {
        String::new()
    };
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

/// The site a host belongs to, as a browser tells sites apart: its public suffix (by the full
/// list, private section included) and one label before it. An address is its own site, and so
/// is a host that is itself a public suffix (`github.io`) or a single label (`localhost`).
pub fn registrable_domain_for_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || host.starts_with('[') || host.parse::<IpAddr>().is_ok() {
        return host;
    }
    SITES
        .domain(host.as_bytes())
        .map(|domain| String::from_utf8_lossy(domain.as_bytes()).into_owned())
        .unwrap_or(host)
}

/// The host, then each parent of it, up to the domain the site's owner registered:
/// `mail.google.com`, then `google.com`; `tokio.readthedocs.io`, then `readthedocs.io`, so a rule
/// for Read the Docs covers every docs site there. Never further, so no rule can cover a public
/// suffix such as `co.uk`. A host that is not under `registrable_domain` yields only itself.
pub(super) fn host_and_parents<'a>(
    host: &'a str,
    registrable_domain: &'a str,
) -> impl Iterator<Item = &'a str> {
    let under = !registrable_domain.is_empty()
        && (host == registrable_domain
            || host.strip_suffix(registrable_domain).is_some_and(|rest| rest.ends_with('.')));
    let top = owner_domain(registrable_domain);
    std::iter::successors(Some(host), move |current| {
        if !under || *current == top {
            return None;
        }
        current.split_once('.').map(|(_, parent)| parent)
    })
}

/// The domain the owner of the site `registrable_domain` registered, by the list's ICANN section:
/// the site itself, except under a private suffix (`readthedocs.io` for `tokio.readthedocs.io`).
/// An address or a name no rule covers is its own: the list's default rule would cut `127.0.0.1`
/// down to `0.1`.
fn owner_domain(registrable_domain: &str) -> &str {
    let private = SITES
        .suffix(registrable_domain.as_bytes())
        .is_some_and(|suffix| suffix.typ() == Some(Type::Private));
    if !private {
        return registrable_domain;
    }
    OWNERS
        .domain(registrable_domain.as_bytes())
        .map(|domain| &registrable_domain[registrable_domain.len() - domain.as_bytes().len()..])
        .unwrap_or(registrable_domain)
}

/// Matches either an exact host or a subdomain of the provided suffix.
pub(super) fn host_matches_suffix(host: &str, suffix: &str) -> bool {
    host == suffix || host.ends_with(&format!(".{suffix}"))
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

// Only unreserved bytes are decoded; encoded slashes and other reserved bytes keep
// their meaning. Uppercase escapes make equivalent percent spellings one key.
fn canonical_path(path: &str) -> String {
    let mut output = String::new();
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(a), Some(b)) =
                ((bytes[index + 1] as char).to_digit(16), (bytes[index + 2] as char).to_digit(16))
        {
            let byte = (a * 16 + b) as u8;
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                output.push(byte as char);
            } else {
                output.push_str(&format!("%{byte:02X}"));
            }
            index += 3;
        } else {
            let ch = path[index..].chars().next().expect("path character");
            output.push(ch);
            index += ch.len_utf8();
        }
    }
    output
}
