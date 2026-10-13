//! Responsibilities: ask an AI service which models it has, what each is for and how much it can
//! read, so Settings offers them instead of asking for an exact id, and the assistant knows the
//! context it has to fit in.
//! Not responsible for: choosing a model, or checking that one answers (`provider.rs`).
//!
//! Each kind of service says it its own way:
//! - LM Studio: `GET /api/v0/models` gives each model's `type` (`llm`, `vlm`, `embeddings`), its
//!   `state` and its context lengths; older versions only have `/v1/models`.
//! - Ollama: `GET /api/tags` names the models, `POST /api/show` gives each one's capabilities and
//!   context length, `GET /api/ps` the context of the ones loaded now.
//! - Anthropic: `GET /v1/models`, every model a chat model.
//! - Google: `GET /v1beta/models`, with the methods each supports and its input limit.
//! - Anything else OpenAI-compatible: `GET /models` under the API root, ids only.

use super::provider::openai_api_root;
use crate::models::{AiModelKind, AiProviderModel, AiRequestFormat};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, time::Duration};

/// A model list is a few small reads on a local server or one on a cloud service.
const LIST_TIMEOUT: Duration = Duration::from_secs(15);

/// The models `format`'s service at `base_url` offers, loaded ones first.
pub async fn list_provider_models(
    format: &AiRequestFormat,
    base_url: Option<&str>,
    api_key: Option<&str>,
) -> Result<Vec<AiProviderModel>> {
    let client = reqwest::Client::builder()
        .timeout(LIST_TIMEOUT)
        .build()
        .context("building the model list client")?;
    let fetch = Fetch { client: &client, api_key };
    let mut models = match format {
        AiRequestFormat::LmStudio => {
            let root = openai_api_root(base_url.context("LM Studio needs an address")?, format);
            match fetch.get(&format!("{}/api/v0/models", origin(&root))).await {
                Ok(body) => parse_lm_studio(&body)?,
                // LM Studio before 0.3.6 has only the OpenAI-compatible list.
                Err(_) => parse_openai(&fetch.get(&format!("{root}/models")).await?)?,
            }
        }
        AiRequestFormat::Ollama => {
            let root = openai_api_root(base_url.context("Ollama needs an address")?, format);
            list_ollama(&fetch, &origin(&root)).await?
        }
        AiRequestFormat::OpenAi => {
            let root = base_url
                .map(|base| openai_api_root(base, format))
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            parse_openai(&fetch.get(&format!("{root}/models")).await?)?
        }
        AiRequestFormat::Anthropic => {
            let root = base_url.unwrap_or("https://api.anthropic.com").trim_end_matches('/');
            let root = root.strip_suffix("/v1").unwrap_or(root);
            parse_anthropic(&fetch.anthropic(&format!("{root}/v1/models?limit=1000")).await?)?
        }
        AiRequestFormat::Google => {
            let root = base_url
                .unwrap_or("https://generativelanguage.googleapis.com")
                .trim_end_matches('/');
            let root = root.strip_suffix("/v1beta").unwrap_or(root);
            parse_google(&fetch.google(&format!("{root}/v1beta/models?pageSize=1000")).await?)?
        }
    };
    models.sort_by(|a, b| {
        b.loaded.unwrap_or(false).cmp(&a.loaded.unwrap_or(false)).then_with(|| a.id.cmp(&b.id))
    });
    Ok(models)
}

/// `scheme://host:port` of an API root: LM Studio's and Ollama's own APIs sit beside `/v1`.
fn origin(root: &str) -> String {
    let after_scheme = root.find("://").map_or(0, |at| at + 3);
    match root[after_scheme..].find('/') {
        Some(path) => root[..after_scheme + path].to_string(),
        None => root.to_string(),
    }
}

struct Fetch<'a> {
    client: &'a reqwest::Client,
    api_key: Option<&'a str>,
}

impl Fetch<'_> {
    /// An OpenAI-style read: the key, when there is one, as a bearer token.
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        let request = self.client.get(url);
        let request = match self.api_key {
            Some(key) => request.bearer_auth(key),
            None => request,
        };
        read(request, url).await
    }

    async fn anthropic(&self, url: &str) -> Result<Vec<u8>> {
        let request = self.client.get(url).header("anthropic-version", "2023-06-01");
        let request = match self.api_key {
            Some(key) => request.header("x-api-key", key),
            None => request,
        };
        read(request, url).await
    }

    /// The key goes in a header, never in the address, so it is not in any log of URLs.
    async fn google(&self, url: &str) -> Result<Vec<u8>> {
        let request = self.client.get(url);
        let request = match self.api_key {
            Some(key) => request.header("x-goog-api-key", key),
            None => request,
        };
        read(request, url).await
    }

    async fn post(&self, url: &str, body: serde_json::Value) -> Result<Vec<u8>> {
        let request =
            self.client.post(url).header("content-type", "application/json").body(body.to_string());
        read(request, url).await
    }
}

async fn read(request: reqwest::RequestBuilder, url: &str) -> Result<Vec<u8>> {
    let response =
        request.send().await.with_context(|| format!("error sending request to {url}"))?;
    let status = response.status();
    let body = response.bytes().await.with_context(|| format!("reading the reply from {url}"))?;
    if !status.is_success() {
        anyhow::bail!(
            "listing models failed with status {}: {}",
            status.as_u16(),
            String::from_utf8_lossy(&body).trim()
        );
    }
    Ok(body.to_vec())
}

/// LM Studio's model list (`/api/v0/models`).
pub(super) fn parse_lm_studio(body: &[u8]) -> Result<Vec<AiProviderModel>> {
    #[derive(Deserialize)]
    struct Reply {
        data: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
        #[serde(rename = "type")]
        kind: Option<String>,
        state: Option<String>,
        loaded_context_length: Option<u32>,
        max_context_length: Option<u32>,
    }
    let reply: Reply = parse(body, "LM Studio's model list")?;
    Ok(reply
        .data
        .into_iter()
        .map(|model| {
            let loaded = model.state.as_deref().map(|state| state == "loaded");
            AiProviderModel {
                kind: match model.kind.as_deref() {
                    Some("llm" | "vlm") => AiModelKind::Chat,
                    Some("embeddings") => AiModelKind::Embedding,
                    _ => AiModelKind::Unknown,
                },
                loaded,
                loaded_context_length: model.loaded_context_length.filter(|_| loaded == Some(true)),
                max_context_length: model.max_context_length,
                id: model.id,
            }
        })
        .collect())
}

/// An OpenAI-compatible `/models`: ids only. An id with "embed" in it is taken for an embedding
/// model (`text-embedding-3-small`, `nomic-embed-text`); the rest are left unknown, not guessed.
pub(super) fn parse_openai(body: &[u8]) -> Result<Vec<AiProviderModel>> {
    #[derive(Deserialize)]
    struct Reply {
        data: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
    }
    let reply: Reply = parse(body, "the model list")?;
    Ok(reply
        .data
        .into_iter()
        .map(|model| AiProviderModel {
            kind: if model.id.to_lowercase().contains("embed") {
                AiModelKind::Embedding
            } else {
                AiModelKind::Unknown
            },
            id: model.id,
            loaded: None,
            loaded_context_length: None,
            max_context_length: None,
        })
        .collect())
}

pub(super) fn parse_anthropic(body: &[u8]) -> Result<Vec<AiProviderModel>> {
    #[derive(Deserialize)]
    struct Reply {
        data: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
    }
    let reply: Reply = parse(body, "Anthropic's model list")?;
    Ok(reply
        .data
        .into_iter()
        .map(|model| AiProviderModel {
            id: model.id,
            kind: AiModelKind::Chat,
            loaded: None,
            loaded_context_length: None,
            max_context_length: None,
        })
        .collect())
}

pub(super) fn parse_google(body: &[u8]) -> Result<Vec<AiProviderModel>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Reply {
        #[serde(default)]
        models: Vec<Model>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Model {
        name: String,
        #[serde(default)]
        supported_generation_methods: Vec<String>,
        input_token_limit: Option<u32>,
    }
    let reply: Reply = parse(body, "Google's model list")?;
    Ok(reply
        .models
        .into_iter()
        .map(|model| {
            let supports =
                |method: &str| model.supported_generation_methods.iter().any(|m| m == method);
            AiProviderModel {
                kind: if supports("embedContent") {
                    AiModelKind::Embedding
                } else if supports("generateContent") {
                    AiModelKind::Chat
                } else {
                    AiModelKind::Unknown
                },
                id: model.name.strip_prefix("models/").unwrap_or(&model.name).to_string(),
                loaded: None,
                loaded_context_length: None,
                max_context_length: model.input_token_limit,
            }
        })
        .collect())
}

async fn list_ollama(fetch: &Fetch<'_>, origin: &str) -> Result<Vec<AiProviderModel>> {
    let names = parse_ollama_tags(&fetch.get(&format!("{origin}/api/tags")).await?)?;
    // `/api/ps` is only the models in memory; an older Ollama without it lists none as loaded.
    let loaded = match fetch.get(&format!("{origin}/api/ps")).await {
        Ok(body) => parse_ollama_ps(&body).unwrap_or_default(),
        Err(_) => HashMap::new(),
    };
    let mut models = Vec::with_capacity(names.len());
    for name in names {
        let shown = fetch
            .post(&format!("{origin}/api/show"), serde_json::json!({ "model": name }))
            .await
            .ok()
            .and_then(|body| parse_ollama_show(&body).ok());
        let (kind, max_context_length) = shown.unwrap_or((AiModelKind::Unknown, None));
        let in_memory = loaded.get(&name);
        models.push(AiProviderModel {
            loaded: Some(in_memory.is_some()),
            loaded_context_length: in_memory.copied().flatten(),
            max_context_length,
            kind,
            id: name,
        });
    }
    Ok(models)
}

pub(super) fn parse_ollama_tags(body: &[u8]) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        models: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        name: String,
    }
    let reply: Reply = parse(body, "Ollama's model list")?;
    Ok(reply.models.into_iter().map(|model| model.name).collect())
}

/// Ollama's models in memory, each with the context it was loaded with when Ollama says.
pub(super) fn parse_ollama_ps(body: &[u8]) -> Result<HashMap<String, Option<u32>>> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        models: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        name: String,
        context_length: Option<u32>,
    }
    let reply: Reply = parse(body, "Ollama's loaded models")?;
    Ok(reply.models.into_iter().map(|model| (model.name, model.context_length)).collect())
}

/// What `/api/show` says a model is for, and the most it can read (`<arch>.context_length`).
pub(super) fn parse_ollama_show(body: &[u8]) -> Result<(AiModelKind, Option<u32>)> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        capabilities: Vec<String>,
        #[serde(default)]
        model_info: HashMap<String, serde_json::Value>,
    }
    let reply: Reply = parse(body, "Ollama's model details")?;
    let has = |capability: &str| reply.capabilities.iter().any(|c| c == capability);
    let kind = if has("embedding") {
        AiModelKind::Embedding
    } else if has("completion") {
        AiModelKind::Chat
    } else {
        AiModelKind::Unknown
    };
    let context = reply
        .model_info
        .iter()
        .find(|(key, _)| key.ends_with(".context_length"))
        .and_then(|(_, value)| value.as_u64())
        .and_then(|length| u32::try_from(length).ok());
    Ok((kind, context))
}

fn parse<'a, T: Deserialize<'a>>(body: &'a [u8], what: &str) -> Result<T> {
    serde_json::from_slice(body).with_context(|| {
        let text = String::from_utf8_lossy(body);
        let shown: String = text.trim().chars().take(300).collect();
        format!("{what} could not be read: {shown}")
    })
}
