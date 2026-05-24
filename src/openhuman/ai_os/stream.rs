//! SSE streaming endpoint for ai_os chat.
//!
//! `POST /ai-os/stream` accepts a JSON body with `conversation_id` and
//! `content`, buffers the full LLM response via `stream_chat_with_history`,
//! then emits each chunk as an SSE `delta` event followed by a final `done`
//! event carrying usage stats.

use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::core::types::AppState;
use crate::openhuman::providers::compatible::{AuthStyle, OpenAiCompatibleProvider};
use crate::openhuman::providers::traits::{
    ChatMessage, Provider, StreamOptions, UsageInfo,
};

use super::store;
use super::types::{AiMessage, ProviderKind};

// ─── request body ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StreamRequest {
    pub conversation_id: String,
    pub content: String,
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn effective_base_url(provider: &super::types::UserProvider) -> String {
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

// ─── handler ─────────────────────────────────────────────────────────────────

/// `POST /ai-os/stream`
///
/// Streams an LLM chat response as Server-Sent Events.
///
/// Events:
/// - `delta`: text chunk from the assistant
/// - `done`: final event with `{ input_tokens, output_tokens, cost_usd }`
/// - `error`: human-readable error string (stream ends after this)
pub async fn stream_handler(
    State(_state): State<AppState>,
    Json(req): Json<StreamRequest>,
) -> impl IntoResponse {
    tracing::debug!(
        conversation_id = %req.conversation_id,
        "[ai_os][stream] stream_handler: entry"
    );

    // ── load config ──────────────────────────────────────────────────────────
    let config = match crate::openhuman::config::load_config_with_timeout().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[ai_os][stream] config load failed: {e}");
            let stream = futures_util::stream::once(async move {
                Ok::<Event, std::convert::Infallible>(
                    Event::default()
                        .event("error")
                        .data(format!("config error: {e}")),
                )
            });
            return Sse::new(stream)
                .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
                .into_response();
        }
    };

    // ── load conversation + provider ─────────────────────────────────────────
    let load_result = store::with_connection(&config, |conn| {
        let conv = store::conversation_get(conn, &req.conversation_id)
            .map_err(|e| anyhow::anyhow!("conversation_get: {e}"))?
            .ok_or_else(|| {
                anyhow::anyhow!("conversation not found: {}", req.conversation_id)
            })?;
        let provider = store::provider_get(conn, &conv.provider_id)
            .map_err(|e| anyhow::anyhow!("provider_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("provider not found: {}", conv.provider_id))?;
        let history = store::messages_for_conversation(conn, &req.conversation_id)
            .map_err(|e| anyhow::anyhow!("messages_for_conversation: {e}"))?;
        Ok((conv, provider, history))
    });

    let (conv, provider, history) = match load_result {
        Ok(v) => v,
        Err(e) => {
            let msg = e.to_string();
            tracing::error!("[ai_os][stream] load failed: {msg}");
            let stream = futures_util::stream::once(async move {
                Ok::<Event, std::convert::Infallible>(
                    Event::default().event("error").data(msg),
                )
            });
            return Sse::new(stream)
                .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
                .into_response();
        }
    };

    // ── build message list ───────────────────────────────────────────────────
    let mut messages: Vec<ChatMessage> = history
        .iter()
        .map(|m| ChatMessage {
            id: Some(m.id.clone()),
            role: m.role.clone(),
            content: m.content.clone(),
            extra_metadata: None,
        })
        .collect();

    let user_msg_id = Uuid::new_v4().to_string();
    messages.push(ChatMessage {
        id: Some(user_msg_id.clone()),
        role: "user".into(),
        content: req.content.clone(),
        extra_metadata: None,
    });

    // ── persist user message immediately ────────────────────────────────────
    let now = now_secs();
    let user_ai_msg = AiMessage {
        id: user_msg_id.clone(),
        conversation_id: req.conversation_id.clone(),
        role: "user".into(),
        content: req.content.clone(),
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
        created_at: now,
    };
    let conversation_id_for_save = req.conversation_id.clone();

    if let Err(e) = store::with_connection(&config, |conn| {
        store::message_insert(conn, &user_ai_msg)
            .map_err(|e| anyhow::anyhow!("message_insert (user): {e}"))
    }) {
        tracing::warn!("[ai_os][stream] failed to save user message: {e}");
    }

    // ── build provider + collect streaming chunks ────────────────────────────
    let base_url = effective_base_url(&provider);
    let p = OpenAiCompatibleProvider::new(
        &provider.name,
        &base_url,
        provider.api_key.as_deref(),
        AuthStyle::Bearer,
    );

    let model = conv.model.clone();

    tracing::debug!(
        conversation_id = %req.conversation_id,
        model = %model,
        message_count = messages.len(),
        "[ai_os][stream] starting stream_chat_with_history"
    );

    // Buffer all chunks (simple approach: collect before emitting SSE).
    // This allows us to save the assistant message reliably after streaming.
    let mut chunk_stream =
        p.stream_chat_with_history(&messages, &model, 0.7, StreamOptions::new(true));

    let mut full_text = String::new();
    let mut sse_events: Vec<Event> = Vec::new();
    let mut had_error: Option<String> = None;

    while let Some(chunk_result) = chunk_stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if !chunk.delta.is_empty() {
                    tracing::trace!(
                        delta_len = chunk.delta.len(),
                        "[ai_os][stream] chunk delta"
                    );
                    full_text.push_str(&chunk.delta);
                    sse_events.push(
                        Event::default()
                            .event("delta")
                            .data(chunk.delta.clone()),
                    );
                }
                if chunk.is_final {
                    tracing::debug!(
                        total_chars = full_text.len(),
                        "[ai_os][stream] received final chunk"
                    );
                }
            }
            Err(e) => {
                tracing::error!("[ai_os][stream] stream error: {e}");
                had_error = Some(e.to_string());
                break;
            }
        }
    }

    // ── persist assistant message ────────────────────────────────────────────
    let usage = UsageInfo::default();
    if had_error.is_none() && !full_text.is_empty() {
        let assistant_msg = AiMessage {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id_for_save.clone(),
            role: "assistant".into(),
            content: full_text.clone(),
            input_tokens: usage.input_tokens as i64,
            output_tokens: usage.output_tokens as i64,
            cost_usd: usage.charged_amount_usd,
            created_at: now + 1,
        };
        if let Err(e) = store::with_connection(&config, |conn| {
            store::message_insert(conn, &assistant_msg)
                .map_err(|e| anyhow::anyhow!("message_insert (assistant): {e}"))?;
            conn.execute(
                "UPDATE ai_os_conversations SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now + 1, conversation_id_for_save],
            )
            .map_err(|e| anyhow::anyhow!("update conversation updated_at: {e}"))?;
            Ok(())
        }) {
            tracing::warn!("[ai_os][stream] failed to save assistant message: {e}");
        }
    }

    // ── append final done/error event ────────────────────────────────────────
    if let Some(err_msg) = had_error {
        sse_events.push(Event::default().event("error").data(err_msg));
    } else {
        let done_payload = json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "cost_usd": usage.charged_amount_usd,
        })
        .to_string();
        sse_events.push(Event::default().event("done").data(done_payload));
    }

    tracing::debug!(
        conversation_id = %req.conversation_id,
        event_count = sse_events.len(),
        "[ai_os][stream] stream_handler: emitting events"
    );

    let stream = futures_util::stream::iter(
        sse_events
            .into_iter()
            .map(|e| Ok::<Event, std::convert::Infallible>(e)),
    );

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response()
}
