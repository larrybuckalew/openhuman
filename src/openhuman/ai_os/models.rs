use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::openhuman::ai_os::types::{ProviderKind, UserProvider};

// ─── types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_length: Option<i64>,
    pub description: Option<String>,
}

// ─── internal response shapes ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

// ─── provider detection ───────────────────────────────────────────────────────

fn is_ollama(provider: &UserProvider) -> bool {
    let url_lower = provider.base_url.to_lowercase();
    let has_local = url_lower.contains("localhost") || url_lower.contains("127.0.0.1");
    let has_no_key = provider.api_key.as_deref().map(|k| k.is_empty()).unwrap_or(true);
    has_local && has_no_key
}

// ─── static model lists ───────────────────────────────────────────────────────

fn anthropic_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "claude-opus-4-7".to_string(),
            name: "Claude Opus 4.7".to_string(),
            context_length: Some(200_000),
            description: None,
        },
        ModelInfo {
            id: "claude-sonnet-4-6".to_string(),
            name: "Claude Sonnet 4.6".to_string(),
            context_length: Some(200_000),
            description: None,
        },
        ModelInfo {
            id: "claude-haiku-4-5-20251001".to_string(),
            name: "Claude Haiku 4.5".to_string(),
            context_length: Some(200_000),
            description: None,
        },
        ModelInfo {
            id: "claude-opus-4-5".to_string(),
            name: "Claude Opus 4.5".to_string(),
            context_length: Some(200_000),
            description: None,
        },
        ModelInfo {
            id: "claude-3-7-sonnet-20250219".to_string(),
            name: "Claude 3.7 Sonnet".to_string(),
            context_length: Some(200_000),
            description: None,
        },
        ModelInfo {
            id: "claude-3-5-haiku-20241022".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
            context_length: Some(200_000),
            description: None,
        },
    ]
}

fn google_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gemini-2.0-flash".to_string(),
            name: "Gemini 2.0 Flash".to_string(),
            context_length: Some(1_000_000),
            description: None,
        },
        ModelInfo {
            id: "gemini-2.0-flash-lite".to_string(),
            name: "Gemini 2.0 Flash Lite".to_string(),
            context_length: Some(1_000_000),
            description: None,
        },
        ModelInfo {
            id: "gemini-1.5-pro".to_string(),
            name: "Gemini 1.5 Pro".to_string(),
            context_length: Some(1_000_000),
            description: None,
        },
        ModelInfo {
            id: "gemini-1.5-flash".to_string(),
            name: "Gemini 1.5 Flash".to_string(),
            context_length: Some(1_000_000),
            description: None,
        },
    ]
}

// ─── public API ───────────────────────────────────────────────────────────────

/// List available models for a given provider.
///
/// For `Anthropic` and `Google` providers, returns a curated static list since
/// their APIs require special credentials or headers to enumerate models.
///
/// For `OpenAiCompatible` providers, queries the `/models` endpoint (or Ollama's
/// `/api/tags` for local instances). On any network or auth failure the function
/// falls back to returning an empty `Vec` rather than propagating an error.
pub async fn list_models(provider: &UserProvider) -> Vec<ModelInfo> {
    tracing::debug!(
        provider_id = %provider.id,
        kind = ?provider.kind,
        "[ai_os][models] list_models: entry"
    );

    match provider.kind {
        ProviderKind::Anthropic => {
            tracing::debug!(
                provider_id = %provider.id,
                "[ai_os][models] list_models: returning static Anthropic model list"
            );
            anthropic_models()
        }
        ProviderKind::Google => {
            tracing::debug!(
                provider_id = %provider.id,
                "[ai_os][models] list_models: returning static Google model list"
            );
            google_models()
        }
        ProviderKind::OpenAiCompatible => {
            fetch_openai_compatible_models(provider).await
        }
    }
}

async fn fetch_openai_compatible_models(provider: &UserProvider) -> Vec<ModelInfo> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(
                provider_id = %provider.id,
                error = %e,
                "[ai_os][models] failed to build reqwest client"
            );
            return vec![];
        }
    };

    if is_ollama(provider) {
        fetch_ollama_models(&client, provider).await
    } else {
        fetch_openai_models(&client, provider).await
    }
}

async fn fetch_openai_models(client: &reqwest::Client, provider: &UserProvider) -> Vec<ModelInfo> {
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));

    tracing::debug!(
        provider_id = %provider.id,
        url = %url,
        "[ai_os][models] fetch_openai_models: GET {url}"
    );

    let mut req = client.get(&url);
    if let Some(ref key) = provider.api_key {
        if !key.is_empty() {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(
                provider_id = %provider.id,
                error = %e,
                "[ai_os][models] fetch_openai_models: network error, returning empty list"
            );
            return vec![];
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(
            provider_id = %provider.id,
            status = %resp.status(),
            "[ai_os][models] fetch_openai_models: non-success status, returning empty list"
        );
        return vec![];
    }

    let parsed: OpenAiModelsResponse = match resp.json().await {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(
                provider_id = %provider.id,
                error = %e,
                "[ai_os][models] fetch_openai_models: parse error, returning empty list"
            );
            return vec![];
        }
    };

    tracing::debug!(
        provider_id = %provider.id,
        count = parsed.data.len(),
        "[ai_os][models] fetch_openai_models: ok"
    );

    parsed
        .data
        .into_iter()
        .map(|m| ModelInfo {
            name: m.id.clone(),
            id: m.id,
            context_length: None,
            description: None,
        })
        .collect()
}

async fn fetch_ollama_models(client: &reqwest::Client, provider: &UserProvider) -> Vec<ModelInfo> {
    // Strip the /v1 suffix if present to get the Ollama root URL
    let base = provider.base_url.trim_end_matches('/');
    let root = if base.ends_with("/v1") {
        &base[..base.len() - 3]
    } else {
        base
    };
    let url = format!("{root}/api/tags");

    tracing::debug!(
        provider_id = %provider.id,
        url = %url,
        "[ai_os][models] fetch_ollama_models: GET {url}"
    );

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(
                provider_id = %provider.id,
                error = %e,
                "[ai_os][models] fetch_ollama_models: network error, returning empty list"
            );
            return vec![];
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(
            provider_id = %provider.id,
            status = %resp.status(),
            "[ai_os][models] fetch_ollama_models: non-success status, returning empty list"
        );
        return vec![];
    }

    let parsed: OllamaTagsResponse = match resp.json().await {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(
                provider_id = %provider.id,
                error = %e,
                "[ai_os][models] fetch_ollama_models: parse error, returning empty list"
            );
            return vec![];
        }
    };

    tracing::debug!(
        provider_id = %provider.id,
        count = parsed.models.len(),
        "[ai_os][models] fetch_ollama_models: ok"
    );

    parsed
        .models
        .into_iter()
        .map(|m| {
            // Strip ":latest" (or any tag) for a cleaner display name
            let display_name = m.name
                .split_once(':')
                .map(|(base, _tag)| base.to_string())
                .unwrap_or_else(|| m.name.clone());
            ModelInfo {
                id: m.name,
                name: display_name,
                context_length: None,
                description: None,
            }
        })
        .collect()
}
