//! Responsibilities: choose model-family prompts and version the embedded document format.
//! Not responsible for: provider selection, transport, vector-space fingerprints or page extraction.

use super::VectorStore;
use crate::config::ProjectPaths;
use crate::enrichment::EmbeddingDocument;

/// Version 1 embedded visit headers; version 2 embeds only page meaning.
pub(crate) const EMBEDDED_TEXT_VERSION: u32 = 2;

const SEARCH_TASK: &str =
    "Given a search query, retrieve the web pages from the user's browsing history that match it";

/// Model names, including aliases served by different providers, share the same prompt contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptProfile {
    Plain,
    Qwen3,
    EmbeddingGemma,
    Nomic,
    E5Instruct,
    E5,
    Mxbai,
}

impl PromptProfile {
    /// Resolve from the model alone so changing servers cannot change model inputs.
    pub(crate) fn for_model(model: &str) -> Self {
        let model = model.to_ascii_lowercase();
        if model.contains("qwen3-embedding") {
            Self::Qwen3
        } else if model.contains("embeddinggemma") {
            Self::EmbeddingGemma
        } else if model.contains("nomic-embed-text") {
            Self::Nomic
        } else if model.split(['/', ':', '-', '_', '.']).any(|part| part == "e5") {
            if model.contains("instruct") { Self::E5Instruct } else { Self::E5 }
        } else if model.contains("mxbai-embed-large") {
            Self::Mxbai
        } else {
            Self::Plain
        }
    }

    /// Query-only prompt changes must leave an existing document index current.
    pub(crate) fn document_format(self) -> &'static str {
        match self {
            Self::EmbeddingGemma => "embeddinggemma",
            Self::Nomic => "nomic",
            Self::E5 => "e5",
            _ => "plain",
        }
    }

    /// Called only at the single query dispatch boundary, before choosing a transport.
    pub(crate) fn render_query(self, query: &str) -> String {
        match self {
            Self::Plain => query.to_owned(),
            Self::Qwen3 => format!("Instruct: {SEARCH_TASK}\nQuery:{query}"),
            Self::EmbeddingGemma => format!("task: search result | query: {query}"),
            Self::Nomic => format!("search_query: {query}"),
            Self::E5Instruct => format!("Instruct: {SEARCH_TASK}\nQuery: {query}"),
            Self::E5 => format!("query: {query}"),
            Self::Mxbai => {
                format!("Represent this sentence for searching relevant passages: {query}")
            }
        }
    }

    /// Render while page parts are available; transports must not add document prompts.
    pub(crate) fn render_document(self, document: &EmbeddingDocument) -> String {
        if self == Self::EmbeddingGemma {
            return format!(
                "title: {} | text: {}",
                document.title.as_deref().unwrap_or("none"),
                document.body
            );
        }
        let prefix = match self {
            Self::Nomic => "search_document: ",
            Self::E5 => "passage: ",
            _ => "",
        };
        let mut text = String::with_capacity(
            prefix.len()
                + document.title.as_ref().map_or(0, |title| title.len() + 1)
                + document.body.len(),
        );
        text.push_str(prefix);
        if let Some(title) = &document.title {
            text.push_str(title);
            if !document.body.is_empty() {
                text.push('\n');
            }
        }
        text.push_str(&document.body);
        text
    }
}

/// The oldest vectors determine a store's text format, independently of its vector space.
pub(crate) fn embedded_text_tag(model: &str) -> String {
    format!("{EMBEDDED_TEXT_VERSION}/{}", PromptProfile::for_model(model).document_format())
}

/// Read outside database closures; absence means there is no index to mark outdated.
pub(crate) fn text_format_is_outdated(
    paths: &ProjectPaths,
    provider: &str,
    model: &str,
) -> anyhow::Result<bool> {
    Ok(VectorStore::for_provider(paths, provider, model).read_header()?.is_some_and(|header| {
        header.embedded_text.as_deref() != Some(embedded_text_tag(model).as_str())
    }))
}

#[cfg(test)]
#[path = "embedding_prompt_tests.rs"]
mod tests;
