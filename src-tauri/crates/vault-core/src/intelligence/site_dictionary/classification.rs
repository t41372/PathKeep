//! Per-visit site classification and search-query extraction.
//!
//! ## Responsibilities
//! - Apply visit-taxonomy analysis plus site-dictionary overrides to one visit.
//! - Extract search-engine identity and normalized query text from canonical
//!   search terms, custom rules, or generic URL analysis.
//! - Provide stable display-name helpers for domain and engine aggregate reads.
//!
//! ## Not responsible for
//! - Loading overrides or search rules from SQLite.
//! - Defining the underlying taxonomy rule packs.
//! - Persisting visit-derived facts or daily rollups.
//!
//! ## Dependencies
//! - `visit_taxonomy` for normalized URL and taxonomy evidence.
//! - `reqwest::Url` parsing for host/path/query matching.
//! - Site-dictionary type contracts from sibling modules.
//!
//! ## Performance notes
//! - Callers should pass preloaded `overrides` and `search_rules`; this module
//!   performs no database I/O and is safe for large rebuild loops.
//! - Rule matching is linear in the small enabled-rule set. Do not grow
//!   built-in/custom rules into visit-scale data without replacing this matcher.

use super::types::{
    SearchEngineRuleConfig, SearchQueryMatch, SiteDictionaryEntry, SiteDictionaryOverride,
};
use crate::visit_taxonomy::{
    DomainCategory, PageCategory, VisitAnalysisInput, analyze_visit, registrable_domain_for_host,
};

/// Classifies one visible archive visit into the normalized site-dictionary row
/// used by Core Intelligence rebuild stages.
pub(crate) fn classify_visit(
    url: &str,
    title: Option<&str>,
    query: Option<&str>,
    has_canonical_search_term: bool,
    external_referrer_url: Option<&str>,
    from_visit: Option<i64>,
    overrides: &[SiteDictionaryOverride],
    search_rules: &[SearchEngineRuleConfig],
) -> SiteDictionaryEntry {
    let parsed_url = reqwest::Url::parse(url).ok();
    let host = parsed_url
        .as_ref()
        .and_then(|parsed| {
            parsed.host_str().map(|value| value.trim_end_matches('.').to_ascii_lowercase())
        })
        .unwrap_or_default();
    let registrable_domain_hint = registrable_domain_for_host(&host);
    let taxonomy_overrides =
        overrides.iter().filter_map(SiteDictionaryOverride::taxonomy_override).collect::<Vec<_>>();
    let analysis = analyze_visit(
        VisitAnalysisInput {
            url,
            title,
            query,
            has_canonical_search_term,
            external_referrer_url,
            from_visit,
        },
        &taxonomy_overrides,
    );
    let normalized_url = analysis.normalized_url;
    let registrable_domain = normalized_url
        .as_ref()
        .map(|value| value.registrable_domain.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or(registrable_domain_hint);
    let canonical_url = normalized_url
        .as_ref()
        .map(|value| value.canonical_url.clone())
        .unwrap_or_else(|| url.to_string());
    let matched_search_rule = parsed_url
        .as_ref()
        .and_then(|parsed| match_search_engine_rule(parsed, &registrable_domain, search_rules));
    let generic_search_query = normalized_url.as_ref().and_then(|value| value.search_query.clone());
    let search_query = query
        .map(normalize_query)
        .filter(|value| !value.is_empty())
        .or_else(|| matched_search_rule.as_ref().map(|rule| normalize_query(&rule.query)))
        .or(generic_search_query);
    let generic_search_engine = search_query
        .as_ref()
        .and(normalized_url.as_ref())
        .map(|value| search_engine_id(&value.registrable_domain, &value.host));
    let matched_override = overrides
        .iter()
        .find(|override_rule| override_rule.matches(url, &registrable_domain, &host));

    SiteDictionaryEntry {
        canonical_url,
        registrable_domain: registrable_domain.clone(),
        domain_category: domain_category_key(analysis.taxonomy.domain_category).to_string(),
        page_category: page_category_key(analysis.taxonomy.page_category).to_string(),
        search_engine: matched_override
            .and_then(|override_rule| override_rule.search_engine.clone())
            .or_else(|| matched_search_rule.as_ref().map(|rule| rule.engine_id.clone()))
            .or(generic_search_engine),
        search_query,
        evidence_tier: analysis.evidence.tier.as_str().to_string(),
        taxonomy_source: analysis.taxonomy.source.as_str().to_string(),
        taxonomy_pack: analysis.taxonomy.rule_pack,
        taxonomy_version: Some(analysis.taxonomy.version),
        display_name: matched_override
            .and_then(|override_rule| override_rule.display_name.clone())
            .or_else(|| matched_search_rule.as_ref().map(|rule| rule.display_name.clone()))
            .or_else(|| display_name_for_domain(&registrable_domain)),
        is_noisy: matched_override.is_some_and(|override_rule| override_rule.is_noisy),
    }
}

/// Canonicalizes a search query for grouping and comparison. It lowercases and
/// collapses whitespace but intentionally keeps punctuation so exact technical
/// searches such as `sqlite-wal` remain distinguishable.
pub(crate) fn normalize_query(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_lowercase()
}

/// Returns user-facing labels for common domains when no Settings rule or
/// override supplies a display name.
pub(crate) fn display_name_for_domain(domain: &str) -> Option<String> {
    match domain {
        // Developer
        "github.com" => Some("GitHub".to_string()),
        "gitlab.com" => Some("GitLab".to_string()),
        "stackoverflow.com" => Some("Stack Overflow".to_string()),
        "npmjs.com" => Some("npm".to_string()),
        "crates.io" => Some("crates.io".to_string()),
        "docs.rs" => Some("docs.rs".to_string()),
        "developer.mozilla.org" | "mozilla.org" => Some("MDN".to_string()),
        "w3schools.com" => Some("W3Schools".to_string()),
        "codepen.io" => Some("CodePen".to_string()),
        "vercel.com" => Some("Vercel".to_string()),
        "netlify.com" => Some("Netlify".to_string()),
        "heroku.com" => Some("Heroku".to_string()),
        "docker.com" => Some("Docker".to_string()),
        "kubernetes.io" => Some("Kubernetes".to_string()),
        "pypi.org" => Some("PyPI".to_string()),
        "rubygems.org" => Some("RubyGems".to_string()),
        "bitbucket.org" => Some("Bitbucket".to_string()),
        "sqlite.org" => Some("SQLite".to_string()),
        // Social
        "reddit.com" => Some("Reddit".to_string()),
        "twitter.com" | "x.com" => Some("X (Twitter)".to_string()),
        "facebook.com" => Some("Facebook".to_string()),
        "instagram.com" => Some("Instagram".to_string()),
        "linkedin.com" => Some("LinkedIn".to_string()),
        "discord.com" => Some("Discord".to_string()),
        "slack.com" => Some("Slack".to_string()),
        "mastodon.social" => Some("Mastodon".to_string()),
        "threads.net" => Some("Threads".to_string()),
        "pinterest.com" => Some("Pinterest".to_string()),
        "tumblr.com" => Some("Tumblr".to_string()),
        // Knowledge
        "wikipedia.org" => Some("Wikipedia".to_string()),
        "medium.com" => Some("Medium".to_string()),
        "substack.com" => Some("Substack".to_string()),
        "quora.com" => Some("Quora".to_string()),
        "zhihu.com" => Some("\u{77E5}\u{4E4E}".to_string()),
        "notion.so" => Some("Notion".to_string()),
        "obsidian.md" => Some("Obsidian".to_string()),
        "wikimedia.org" => Some("Wikimedia".to_string()),
        // Search
        "google.com" => Some("Google".to_string()),
        "bing.com" => Some("Bing".to_string()),
        "duckduckgo.com" => Some("DuckDuckGo".to_string()),
        "brave.com" => Some("Brave".to_string()),
        "yahoo.com" => Some("Yahoo".to_string()),
        "yandex.ru" => Some("Yandex".to_string()),
        "ecosia.org" => Some("Ecosia".to_string()),
        "startpage.com" => Some("Startpage".to_string()),
        // Video / Media
        "youtube.com" => Some("YouTube".to_string()),
        "bilibili.com" => Some("BiliBili".to_string()),
        "twitch.tv" => Some("Twitch".to_string()),
        "netflix.com" => Some("Netflix".to_string()),
        "spotify.com" => Some("Spotify".to_string()),
        "vimeo.com" => Some("Vimeo".to_string()),
        "douyin.com" => Some("\u{6296}\u{97F3}".to_string()),
        "iqiyi.com" => Some("iQIYI".to_string()),
        "soundcloud.com" => Some("SoundCloud".to_string()),
        "disneyplus.com" => Some("Disney+".to_string()),
        // Shopping
        "amazon.com" => Some("Amazon".to_string()),
        "ebay.com" => Some("eBay".to_string()),
        "taobao.com" => Some("\u{6DD8}\u{5BF6}".to_string()),
        "jd.com" => Some("\u{4EAC}\u{6771}".to_string()),
        "shopify.com" => Some("Shopify".to_string()),
        "etsy.com" => Some("Etsy".to_string()),
        "aliexpress.com" => Some("AliExpress".to_string()),
        "walmart.com" => Some("Walmart".to_string()),
        // News
        "cnn.com" => Some("CNN".to_string()),
        "bbc.com" | "bbc.co.uk" => Some("BBC".to_string()),
        "nytimes.com" => Some("NYT".to_string()),
        "theguardian.com" => Some("The Guardian".to_string()),
        "reuters.com" => Some("Reuters".to_string()),
        "news.ycombinator.com" | "ycombinator.com" => Some("Hacker News".to_string()),
        "washingtonpost.com" => Some("Washington Post".to_string()),
        "apnews.com" => Some("AP News".to_string()),
        // AI
        "openai.com" => Some("OpenAI".to_string()),
        "anthropic.com" => Some("Anthropic".to_string()),
        "huggingface.co" => Some("Hugging Face".to_string()),
        "chatgpt.com" => Some("ChatGPT".to_string()),
        "claude.ai" => Some("Claude".to_string()),
        "gemini.google.com" => Some("Gemini".to_string()),
        "perplexity.ai" => Some("Perplexity".to_string()),
        // Cloud / SaaS
        "aws.amazon.com" => Some("AWS".to_string()),
        "cloud.google.com" => Some("Google Cloud".to_string()),
        "azure.microsoft.com" => Some("Azure".to_string()),
        "figma.com" => Some("Figma".to_string()),
        "canva.com" => Some("Canva".to_string()),
        "dropbox.com" => Some("Dropbox".to_string()),
        "drive.google.com" => Some("Google Drive".to_string()),
        "zoom.us" => Some("Zoom".to_string()),
        "atlassian.com" => Some("Atlassian".to_string()),
        // Chinese sites
        "weibo.com" => Some("\u{5FAE}\u{535A}".to_string()),
        "csdn.net" => Some("CSDN".to_string()),
        "juejin.cn" => Some("\u{6398}\u{91D1}".to_string()),
        "segmentfault.com" => Some("SegmentFault".to_string()),
        "douban.com" => Some("\u{8C46}\u{74E3}".to_string()),
        "xiaohongshu.com" => Some("\u{5C0F}\u{7D05}\u{66F8}".to_string()),
        "baidu.com" => Some("\u{767E}\u{5EA6}".to_string()),
        "tencent.com" => Some("\u{9A30}\u{8A0A}".to_string()),
        "aliyun.com" => Some("\u{963F}\u{91CC}\u{96F2}".to_string()),
        "sogou.com" => Some("\u{641C}\u{72D7}".to_string()),
        "so.com" => Some("360 Search".to_string()),
        "bilibili.tv" => Some("BiliBili".to_string()),
        "zhipin.com" => Some("BOSS\u{76F4}\u{8058}".to_string()),
        // Docs / Office
        "docs.google.com" => Some("Google Docs".to_string()),
        "office.com" => Some("Microsoft Office".to_string()),
        "apple.com" => Some("Apple".to_string()),
        "microsoft.com" => Some("Microsoft".to_string()),
        // Misc
        "archive.org" => Some("Internet Archive".to_string()),
        "producthunt.com" => Some("Product Hunt".to_string()),
        "trello.com" => Some("Trello".to_string()),
        "airtable.com" => Some("Airtable".to_string()),
        "stackoverflow.blog" => Some("Stack Overflow Blog".to_string()),
        _ => None,
    }
}

/// Returns display labels for legacy or built-in search engine ids.
pub(crate) fn display_name_for_search_engine(engine: &str) -> Option<String> {
    match engine {
        "amazon" => Some("Amazon".to_string()),
        "baidu" => Some("Baidu".to_string()),
        "bilibili" => Some("BiliBili".to_string()),
        "bing" => Some("Bing".to_string()),
        "brave" => Some("Brave Search".to_string()),
        "duckduckgo" => Some("DuckDuckGo".to_string()),
        "ecosia" => Some("Ecosia".to_string()),
        "github" => Some("GitHub".to_string()),
        "google" => Some("Google".to_string()),
        "perplexity" => Some("Perplexity".to_string()),
        "reddit" => Some("Reddit".to_string()),
        "searx" => Some("SearX".to_string()),
        "sogou" => Some("Sogou".to_string()),
        "so" => Some("360 Search".to_string()),
        "startpage" => Some("Startpage".to_string()),
        "taobao" => Some("Taobao".to_string()),
        "wolfram" => Some("Wolfram Alpha".to_string()),
        "youtube" => Some("YouTube".to_string()),
        "yahoo" => Some("Yahoo".to_string()),
        "yandex" => Some("Yandex".to_string()),
        "zhihu" => Some("Zhihu".to_string()),
        _ => None,
    }
}

/// Derives a stable search-engine id from generic URL analysis when no
/// user-configured rule matched the URL.
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
        "reddit.com" => "reddit",
        "youtube.com" => "youtube",
        "bilibili.com" => "bilibili",
        "github.com" => "github",
        "amazon.com" => "amazon",
        "taobao.com" => "taobao",
        "zhihu.com" => "zhihu",
        "ecosia.org" => "ecosia",
        "startpage.com" => "startpage",
        "perplexity.ai" => "perplexity",
        "wolframalpha.com" => "wolfram",
        _ if host.contains("searx") => "searx",
        _ => host,
    }
    .to_string()
}

/// Matches one parsed URL against enabled search-engine rules and extracts its
/// query value when the host, path, and parameter key all match.
fn match_search_engine_rule(
    parsed_url: &reqwest::Url,
    registrable_domain: &str,
    search_rules: &[SearchEngineRuleConfig],
) -> Option<SearchQueryMatch> {
    let host = parsed_url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let path = parsed_url.path();
    for rule in search_rules.iter().filter(|rule| rule.enabled) {
        if !host_matches_rule(&host, registrable_domain, &rule.host_pattern)
            || !path_matches_rule(path, rule.path_prefix.as_deref())
        {
            continue;
        }
        let value = parsed_url
            .query_pairs()
            .find(|(key, _)| key.eq_ignore_ascii_case(rule.query_param_key.as_str()))
            .map(|(_, value)| normalize_query_spacing(&value.replace('+', " ")))
            .filter(|value| !value.is_empty())?;
        return Some(SearchQueryMatch {
            engine_id: rule.engine_id.clone(),
            display_name: rule.display_name.clone(),
            query: value,
        });
    }
    None
}

/// Checks a URL host against a rule's host pattern, allowing subdomains and
/// registrable-domain matches.
fn host_matches_rule(host: &str, registrable_domain: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    if pattern.is_empty() {
        return false;
    }
    host == pattern
        || host.ends_with(&format!(".{pattern}"))
        || registrable_domain == pattern
        || registrable_domain.ends_with(&format!(".{pattern}"))
}

/// Applies an optional rule path prefix. Missing prefixes intentionally match
/// any path because engines such as DuckDuckGo use `/`.
fn path_matches_rule(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|value| path.starts_with(value))
}

/// Collapses URL-decoded search query spacing without lowercasing; the final
/// canonical lowercasing is handled by `normalize_query`.
fn normalize_query_spacing(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

/// Converts the taxonomy domain enum into the persisted rollup key.
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

/// Converts the taxonomy page enum into the persisted rollup key.
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
    use super::*;

    #[test]
    fn helper_mappings_cover_rule_and_taxonomy_boundaries() {
        assert_eq!(search_engine_id("baidu.com", "www.baidu.com"), "baidu");
        assert_eq!(search_engine_id("bing.com", "www.bing.com"), "bing");
        assert_eq!(search_engine_id("brave.com", "search.brave.com"), "brave");
        assert_eq!(search_engine_id("duckduckgo.com", "duckduckgo.com"), "duckduckgo");
        assert_eq!(search_engine_id("google.com", "www.google.com"), "google");
        assert_eq!(search_engine_id("sogou.com", "www.sogou.com"), "sogou");
        assert_eq!(search_engine_id("so.com", "www.so.com"), "so");
        assert_eq!(search_engine_id("yahoo.com", "search.yahoo.com"), "yahoo");
        assert_eq!(search_engine_id("yandex.ru", "yandex.ru"), "yandex");
        assert_eq!(search_engine_id("example.com", "search.example.com"), "search.example.com");

        assert!(!host_matches_rule("search.example.com", "example.com", "   "));
        assert!(host_matches_rule("search.example.com", "example.com", "example.com."));
        assert!(host_matches_rule(
            "docs.internal.example.com",
            "internal.example.com",
            "example.com"
        ));
        assert!(path_matches_rule("/search/all", None));
        assert!(path_matches_rule("/search/all", Some("/search")));
        assert!(!path_matches_rule("/docs", Some("/search")));
        assert_eq!(normalize_query_spacing("  rust\tcoverage\n gate  "), "rust coverage gate");

        let domain_categories = [
            (DomainCategory::Ai, "ai"),
            (DomainCategory::Community, "community"),
            (DomainCategory::Developer, "developer"),
            (DomainCategory::Docs, "docs"),
            (DomainCategory::Education, "education"),
            (DomainCategory::Entertainment, "entertainment"),
            (DomainCategory::Finance, "finance"),
            (DomainCategory::News, "news"),
            (DomainCategory::Search, "search"),
            (DomainCategory::Shopping, "shopping"),
            (DomainCategory::Social, "social"),
            (DomainCategory::Travel, "travel"),
            (DomainCategory::Video, "video"),
            (DomainCategory::Work, "work"),
            (DomainCategory::Unknown, "unknown"),
        ];
        for (category, expected) in domain_categories {
            assert_eq!(domain_category_key(category), expected);
        }

        let page_categories = [
            (PageCategory::SearchResults, "search_results"),
            (PageCategory::DocsPage, "docs_page"),
            (PageCategory::Repo, "repo"),
            (PageCategory::Issue, "issue"),
            (PageCategory::PullRequest, "pull_request"),
            (PageCategory::ForumThread, "forum_thread"),
            (PageCategory::ProductPage, "product_page"),
            (PageCategory::CategoryPage, "category_page"),
            (PageCategory::VideoPage, "video_page"),
            (PageCategory::ArticlePage, "article_page"),
            (PageCategory::Profile, "profile"),
            (PageCategory::Dashboard, "dashboard"),
            (PageCategory::Home, "home"),
            (PageCategory::Unknown, "unknown"),
        ];
        for (category, expected) in page_categories {
            assert_eq!(page_category_key(category), expected);
        }
    }
}
