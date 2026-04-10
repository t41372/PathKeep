use reqwest::Url;

const SEARCH_QUERY_KEYS: &[&str] =
    &["q", "query", "query_text", "search_query", "p", "wd", "word", "text", "keyword", "k"];
const MULTI_LABEL_PUBLIC_SUFFIXES: &[&str] = &[
    "co.jp", "co.kr", "co.uk", "com.au", "com.cn", "com.hk", "com.sg", "com.tr", "com.tw",
    "net.cn", "org.cn",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedVisitUrl {
    pub canonical_url: String,
    pub host: String,
    pub registrable_domain: String,
    pub path: String,
    pub preserved_query: Vec<(String, String)>,
    pub dropped_tracking_params: Vec<String>,
    pub search_query: Option<String>,
    pub is_search_results: bool,
}

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
        {
            let mut serializer = parsed.query_pairs_mut();
            for (key, value) in &preserved_query {
                serializer.append_pair(key, value);
            }
        }
    }

    Some(NormalizedVisitUrl {
        canonical_url: parsed.to_string(),
        host: host.clone(),
        registrable_domain: registrable_domain_for_host(&host),
        path: parsed.path().to_string(),
        preserved_query,
        dropped_tracking_params,
        search_query: search_query.clone(),
        is_search_results: search_query.is_some() && is_search_engine_host(&host),
    })
}

pub fn extract_search_query_from_url(url: &str) -> Option<String> {
    normalize_visit_url(url).and_then(|value| value.search_query)
}

pub fn registrable_domain_for_url(url: &str) -> Option<String> {
    normalize_visit_url(url).map(|value| value.registrable_domain)
}

pub fn registrable_domain_for_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    let segments = host.split('.').collect::<Vec<_>>();
    if segments.len() <= 2 {
        return host;
    }

    let suffix = format!("{}.{}", segments[segments.len() - 2], segments[segments.len() - 1]);
    if MULTI_LABEL_PUBLIC_SUFFIXES.iter().any(|candidate| candidate.eq_ignore_ascii_case(&suffix))
        && segments.len() >= 3
    {
        return segments[segments.len() - 3..].join(".");
    }

    segments[segments.len() - 2..].join(".")
}

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

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut saw_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !saw_space {
                output.push(' ');
                saw_space = true;
            }
        } else {
            output.push(ch);
            saw_space = false;
        }
    }
    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_search_urls_and_strips_tracking_params() {
        let normalized = normalize_visit_url(
            "https://www.google.com/search?q=sqlite+wal&utm_source=newsletter&gclid=abc123",
        )
        .expect("normalized search url");

        assert_eq!(normalized.host, "www.google.com");
        assert_eq!(normalized.registrable_domain, "google.com");
        assert_eq!(normalized.search_query.as_deref(), Some("sqlite wal"));
        assert!(normalized.is_search_results);
        assert_eq!(normalized.dropped_tracking_params, vec!["utm_source", "gclid"]);
        assert_eq!(normalized.canonical_url, "https://www.google.com/search?q=sqlite+wal");
    }

    #[test]
    fn keeps_semantic_ids_and_extracts_cjk_search_terms() {
        let normalized =
            normalize_visit_url("https://www.baidu.com/s?wd=%E6%9C%AC%E5%9C%B0+AI&spm=track")
                .expect("normalized baidu url");

        assert_eq!(normalized.registrable_domain, "baidu.com");
        assert_eq!(normalized.search_query.as_deref(), Some("本地 AI"));
        assert_eq!(normalized.canonical_url, "https://www.baidu.com/s?wd=%E6%9C%AC%E5%9C%B0+AI");
    }

    #[test]
    fn registrable_domain_handles_common_multi_label_suffixes() {
        assert_eq!(
            registrable_domain_for_url("https://docs.news.bbc.co.uk/path").as_deref(),
            Some("bbc.co.uk")
        );
        assert_eq!(
            registrable_domain_for_url("https://subdomain.example.com.cn/path").as_deref(),
            Some("example.com.cn")
        );
    }

    #[test]
    fn non_search_urls_keep_semantic_query_params() {
        let normalized = normalize_visit_url(
            "https://github.com/example/repo/issues/42?tab=comments&utm_campaign=tracker",
        )
        .expect("normalized issue url");

        assert!(!normalized.is_search_results);
        assert!(normalized.search_query.is_none());
        assert_eq!(normalized.registrable_domain, "github.com");
        assert_eq!(
            normalized.canonical_url,
            "https://github.com/example/repo/issues/42?tab=comments"
        );
    }
}
