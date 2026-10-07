//! Responsibilities: the host and registrable domain a favicon row is filed under, from its page
//! URL, including URLs the visit normalizer rejects.
//! Not responsible for: storing favicons or fetching icons.
//! Failure modes: a favicon filed under the wrong site, or under none when the URL has a host.
use crate::{
    utils::url_domain,
    visit_taxonomy::{normalize_visit_url, registrable_domain_for_host},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaviconUrlMetadata {
    pub(crate) host: Option<String>,
    pub(crate) registrable_domain: Option<String>,
}

pub(crate) fn favicon_url_metadata(page_url: &str) -> FaviconUrlMetadata {
    if let Some(normalized) = normalize_visit_url(page_url) {
        return FaviconUrlMetadata {
            host: Some(normalized.host),
            registrable_domain: Some(normalized.registrable_domain),
        };
    }

    let host = url_domain(page_url)
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return FaviconUrlMetadata { host: None, registrable_domain: None };
    }

    let registrable_domain = registrable_domain_for_host(&host);
    FaviconUrlMetadata {
        host: Some(host),
        registrable_domain: (!registrable_domain.is_empty()).then_some(registrable_domain),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favicon_url_metadata_normalizes_host_and_registrable_domain() {
        assert_eq!(
            favicon_url_metadata("https://docs.news.bbc.co.uk/path"),
            FaviconUrlMetadata {
                host: Some("docs.news.bbc.co.uk".to_string()),
                registrable_domain: Some("bbc.co.uk".to_string()),
            }
        );
        assert_eq!(
            favicon_url_metadata("example.com:443/path"),
            FaviconUrlMetadata {
                host: Some("example.com".to_string()),
                registrable_domain: Some("example.com".to_string()),
            }
        );
        assert_eq!(
            favicon_url_metadata("   "),
            FaviconUrlMetadata { host: None, registrable_domain: None }
        );
    }
}
