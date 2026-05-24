use crate::openhuman::providers::compatible::{AuthStyle, OpenAiCompatibleProvider};
use crate::openhuman::providers::traits::{ChatMessage, ChatRequest, Provider, UsageInfo};

use super::types::{ProviderKind, UserProvider};

/// Send a non-streaming chat request to the given `UserProvider`.
///
/// Returns `(assistant_text, usage)` on success, or a human-readable error string.
pub async fn chat(
    provider: &UserProvider,
    messages: Vec<ChatMessage>,
    model: &str,
) -> Result<(String, UsageInfo), String> {
    tracing::debug!(
        provider_id = %provider.id,
        provider_name = %provider.name,
        model,
        message_count = messages.len(),
        "[ai_os][chat] dispatching chat request"
    );

    let base_url = effective_base_url(provider);
    let p = OpenAiCompatibleProvider::new(
        &provider.name,
        &base_url,
        provider.api_key.as_deref(),
        AuthStyle::Bearer,
    );

    let req = ChatRequest {
        messages: &messages,
        tools: None,
        stream: None,
    };

    match p.chat(req, model, 0.7).await {
        Ok(resp) => {
            let text = resp
                .text
                .unwrap_or_else(|| String::from("(no response text)"));
            let usage = resp.usage.unwrap_or_default();
            tracing::debug!(
                provider_id = %provider.id,
                model,
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                "[ai_os][chat] response received"
            );
            Ok((text, usage))
        }
        Err(err) => {
            tracing::error!(
                provider_id = %provider.id,
                model,
                error = %err,
                "[ai_os][chat] request failed"
            );
            Err(format!("chat error: {err}"))
        }
    }
}

/// Resolve the effective base URL for a provider, applying well-known overrides
/// for Anthropic and Google kinds.
fn effective_base_url(provider: &UserProvider) -> String {
    match &provider.kind {
        ProviderKind::Anthropic => {
            if provider.base_url.is_empty()
                || provider.base_url == "https://api.anthropic.com"
                || provider.base_url == "https://api.anthropic.com/v1"
            {
                "https://api.anthropic.com/v1".into()
            } else {
                provider.base_url.clone()
            }
        }
        ProviderKind::Google => {
            if provider.base_url.is_empty() {
                "https://generativelanguage.googleapis.com/v1beta/openai".into()
            } else {
                provider.base_url.clone()
            }
        }
        ProviderKind::OpenAiCompatible => provider.base_url.clone(),
    }
}
