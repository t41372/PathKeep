//! Site Dictionary and deterministic URL/query normalization helpers.
//!
//! Core Intelligence uses one shared site-dictionary layer so session/trail/
//! rollup queries stay consistent about canonical URLs, registrable domains,
//! search-engine detection, and display aliases.

use crate::deterministic::{DomainCategory, PageCategory, VisitAnalysisInput, analyze_visit};

#[derive(Debug, Clone, Default)]
pub(crate) struct SiteDictionaryEntry {
    pub canonical_url: String,
    pub registrable_domain: String,
    pub domain_category: String,
    pub page_category: String,
    pub search_engine: Option<String>,
    pub search_query: Option<String>,
    pub evidence_tier: String,
    pub taxonomy_source: String,
    pub taxonomy_pack: Option<String>,
    pub taxonomy_version: Option<String>,
    pub display_name: Option<String>,
}

pub(crate) fn classify_visit(
    url: &str,
    title: Option<&str>,
    query: Option<&str>,
    has_canonical_search_term: bool,
    external_referrer_url: Option<&str>,
    from_visit: Option<i64>,
) -> SiteDictionaryEntry {
    let analysis = analyze_visit(
        VisitAnalysisInput {
            url,
            title,
            query,
            has_canonical_search_term,
            external_referrer_url,
            from_visit,
        },
        &[],
    );
    let normalized_url = analysis.normalized_url;
    let registrable_domain =
        normalized_url.as_ref().map(|value| value.registrable_domain.clone()).unwrap_or_default();
    let canonical_url = normalized_url
        .as_ref()
        .map(|value| value.canonical_url.clone())
        .unwrap_or_else(|| url.to_string());
    let search_query = query
        .map(normalize_query)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_url.as_ref().and_then(|value| value.search_query.clone()));
    let search_engine = search_query
        .as_ref()
        .and(normalized_url.as_ref())
        .map(|value| search_engine_id(&value.registrable_domain, &value.host));

    SiteDictionaryEntry {
        canonical_url,
        registrable_domain: registrable_domain.clone(),
        domain_category: domain_category_key(analysis.taxonomy.domain_category).to_string(),
        page_category: page_category_key(analysis.taxonomy.page_category).to_string(),
        search_engine,
        search_query,
        evidence_tier: analysis.evidence.tier.as_str().to_string(),
        taxonomy_source: analysis.taxonomy.source.as_str().to_string(),
        taxonomy_pack: analysis.taxonomy.rule_pack,
        taxonomy_version: Some(analysis.taxonomy.version),
        display_name: display_name_for_domain(&registrable_domain),
    }
}

pub(crate) fn normalize_query(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_lowercase()
}

pub(crate) fn display_name_for_domain(domain: &str) -> Option<String> {
    match domain {
        "bilibili.com" => Some("BiliBili".to_string()),
        "developer.mozilla.org" | "mozilla.org" => Some("MDN".to_string()),
        "github.com" => Some("GitHub".to_string()),
        "google.com" => Some("Google".to_string()),
        "stackoverflow.com" => Some("Stack Overflow".to_string()),
        "sqlite.org" => Some("SQLite".to_string()),
        "youtube.com" => Some("YouTube".to_string()),
        _ => None,
    }
}

pub(crate) fn display_name_for_search_engine(engine: &str) -> Option<String> {
    match engine {
        "baidu" => Some("Baidu".to_string()),
        "bing" => Some("Bing".to_string()),
        "brave" => Some("Brave Search".to_string()),
        "duckduckgo" => Some("DuckDuckGo".to_string()),
        "google" => Some("Google".to_string()),
        "sogou" => Some("Sogou".to_string()),
        "so" => Some("360 Search".to_string()),
        "yahoo" => Some("Yahoo".to_string()),
        "yandex" => Some("Yandex".to_string()),
        _ => None,
    }
}

pub(crate) fn search_engine_id(registrable_domain: &str, host: &str) -> String {
    match registrable_domain {
        "baidu.com" => "baidu",
        "bing.com" => "bing",
        "brave.com" if host == "search.brave.com" => "brave",
        "duckduckgo.com" => "duckduckgo",
        "google.com" => "google",
        "sogou.com" => "sogou",
        "so.com" => "so",
        "yahoo.com" => "yahoo",
        "yandex.ru" => "yandex",
        _ => host,
    }
    .to_string()
}

fn domain_category_key(category: DomainCategory) -> &'static str {
    match category {
        DomainCategory::Ai => "ai",
        DomainCategory::Community => "community",
        DomainCategory::Developer => "developer",
        DomainCategory::Docs => "docs",
        DomainCategory::Education => "education",
        DomainCategory::Entertainment => "entertainment",
        DomainCategory::Finance => "finance",
        DomainCategory::News => "news",
        DomainCategory::Search => "search",
        DomainCategory::Shopping => "shopping",
        DomainCategory::Social => "social",
        DomainCategory::Travel => "travel",
        DomainCategory::Video => "video",
        DomainCategory::Work => "work",
        DomainCategory::Unknown => "unknown",
    }
}

fn page_category_key(category: PageCategory) -> &'static str {
    match category {
        PageCategory::SearchResults => "search_results",
        PageCategory::DocsPage => "docs_page",
        PageCategory::Repo => "repo",
        PageCategory::Issue => "issue",
        PageCategory::PullRequest => "pull_request",
        PageCategory::ForumThread => "forum_thread",
        PageCategory::ProductPage => "product_page",
        PageCategory::CategoryPage => "category_page",
        PageCategory::VideoPage => "video_page",
        PageCategory::ArticlePage => "article_page",
        PageCategory::Profile => "profile",
        PageCategory::Dashboard => "dashboard",
        PageCategory::Home => "home",
        PageCategory::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_visit, normalize_query};

    #[test]
    fn classify_visit_extracts_search_engine_and_query() {
        let entry = classify_visit(
            "https://www.google.com/search?q=sqlite+wal+checkpoint&utm_source=test",
            Some("sqlite wal checkpoint - Google Search"),
            None,
            true,
            None,
            None,
        );
        assert_eq!(entry.registrable_domain, "google.com");
        assert_eq!(entry.search_engine.as_deref(), Some("google"));
        assert_eq!(entry.search_query.as_deref(), Some("sqlite wal checkpoint"));
        assert!(entry.canonical_url.contains("q=sqlite+wal+checkpoint"));
        assert!(!entry.canonical_url.contains("utm_source"));
    }

    #[test]
    fn normalize_query_canonicalizes_whitespace() {
        assert_eq!(normalize_query("  Tauri   v2   Docs "), "tauri v2 docs");
    }
}
