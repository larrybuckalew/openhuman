//! SSE streaming endpoint for ai_os chat.
//!
//! `POST /ai-os/stream` accepts a JSON body with `conversation_id`, `content`,
//! and an optional `model` override. Chunks are emitted in real-time via a
//! tokio channel so the client sees tokens as they arrive.
//!
//! Events:
//! - `delta`: text chunk from the assistant
//! - `done`: final event with `{ input_tokens, output_tokens, cost_usd }`
//! - `error`: human-readable error string (stream ends after this)

use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::core::types::AppState;
use crate::openhuman::providers::compatible::{AuthStyle, OpenAiCompatibleProvider};
use crate::openhuman::providers::traits::{
    ChatMessage, Provider, StreamOptions,
};

use super::store;
use super::types::{AiMessage, ProviderKind};

// ─── request body ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StreamRequest {
    pub conversation_id: String,
    pub content: String,
    /// Optional model override; when absent, the conversation's stored model is used.
    pub model: Option<String>,
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
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
/// Streams an LLM chat response as Server-Sent Events in real-time.
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
        model = ?req.model,
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
    let now = now_ms();
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

    // ── build provider ───────────────────────────────────────────────────────
    let base_url = effective_base_url(&provider);
    let p = OpenAiCompatibleProvider::new(
        &provider.name,
        &base_url,
        provider.api_key.as_deref(),
        AuthStyle::Bearer,
    );

    // Use the per-request model override if provided, otherwise fall back to
    // the conversation's configured model.
    let model = req.model.clone().unwrap_or_else(|| conv.model.clone());

    tracing::debug!(
        conversation_id = %req.conversation_id,
        model = %model,
        message_count = messages.len(),
        "[ai_os][stream] starting real-time stream"
    );

    // ── real-time streaming via channel ──────────────────────────────────────
    let (tx, rx) =
        tokio::sync::mpsc::channel::<Result<Event, std::convert::Infallible>>(64);

    tokio::spawn(async move {
        let mut chunk_stream =
            p.stream_chat_with_history(&messages, &model, 0.7, StreamOptions::new(true));

        let mut full_text = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut cost_usd: f64 = 0.0;
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
                        let event = Event::default()
                            .event("delta")
                            .data(chunk.delta.clone());
                        if tx.send(Ok(event)).await.is_err() {
                            // Client disconnected — stop streaming.
                            tracing::debug!("[ai_os][stream] client disconnected mid-stream");
                            return;
                        }
                    }
                    if chunk.is_final {
                        // token_count is an estimate from StreamChunk; real usage
                        // accumulates here for the done payload.
                        output_tokens = output_tokens.saturating_add(chunk.token_count as u64);
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

        // ── persist assistant message ────────────────────────────────────────
        if had_error.is_none() && !full_text.is_empty() {
            let ts = now_ms();
            let assistant_msg = AiMessage {
                id: Uuid::new_v4().to_string(),
                conversation_id: conversation_id_for_save.clone(),
                role: "assistant".into(),
                content: full_text.clone(),
                input_tokens: input_tokens as i64,
                output_tokens: output_tokens as i64,
                cost_usd,
                // +1 ms to guarantee ordering after the user message.
                created_at: ts + 1,
            };
            if let Err(e) = store::with_connection(&config, |conn| {
                store::message_insert(conn, &assistant_msg)
                    .map_err(|e| anyhow::anyhow!("message_insert (assistant): {e}"))?;
                conn.execute(
                    "UPDATE ai_os_conversations SET updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![ts + 1, conversation_id_for_save],
                )
                .map_err(|e| anyhow::anyhow!("update conversation updated_at: {e}"))?;
                Ok(())
            }) {
                tracing::warn!("[ai_os][stream] failed to save assistant message: {e}");
            }
        }

        // ── send terminal event ──────────────────────────────────────────────
        let terminal = if let Some(err_msg) = had_error {
            Ok(Event::default().event("error").data(err_msg))
        } else {
            let done_payload = json!({
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cost_usd": cost_usd,
            })
            .to_string();
            Ok(Event::default().event("done").data(done_payload))
        };

        // Ignore send error here — if the client is gone we just drop the event.
        let _ = tx.send(terminal).await;

        tracing::debug!(
            conversation_id = %req.conversation_id,
            "[ai_os][stream] stream_handler: task complete"
        );
    });

    Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response()
}
