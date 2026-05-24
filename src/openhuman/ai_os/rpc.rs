use serde_json::{Map, Value};
use uuid::Uuid;

use crate::openhuman::config::Config;
use crate::openhuman::providers::traits::ChatMessage;

use super::store;
use super::types::{AiConversation, AiMessage, ProviderKind, UsageSummary, UserProvider};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn param_str(params: &Map<String, Value>, key: &str) -> Result<String, String> {
    match params.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(format!("param '{key}' must be a string, got {other}")),
        None => Err(format!("missing required param '{key}'")),
    }
}

fn param_str_opt(params: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(format!("param '{key}' must be a string, got {other}")),
    }
}

fn param_bool_opt(params: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(other) => Err(format!("param '{key}' must be a bool, got {other}")),
    }
}

fn param_i64_opt(params: &Map<String, Value>, key: &str) -> Result<Option<i64>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("param '{key}' must be an integer")),
        Some(other) => Err(format!("param '{key}' must be a number, got {other}")),
    }
}

fn parse_provider_kind(s: &str) -> Result<ProviderKind, String> {
    match s {
        "open_ai_compatible" => Ok(ProviderKind::OpenAiCompatible),
        "anthropic" => Ok(ProviderKind::Anthropic),
        "google" => Ok(ProviderKind::Google),
        other => Err(format!(
            "unknown provider kind '{other}'; expected open_ai_compatible, anthropic, or google"
        )),
    }
}

fn load_config_sync() -> Result<Config, String> {
    // Use tokio block_in_place to call async config loader from sync context.
    // In practice these handlers run inside an async runtime, so we spawn a
    // blocking task instead.
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(crate::openhuman::config::load_config_with_timeout())
    })
}

// ─── provider handlers ───────────────────────────────────────────────────────

pub async fn handle_provider_add(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] provider_add");
    let config = crate::openhuman::config::load_config_with_timeout().await?;

    let name = param_str(&params, "name")?;
    let kind_str = param_str(&params, "kind")?;
    let kind = parse_provider_kind(&kind_str)?;
    let base_url = param_str(&params, "base_url")?;
    let api_key = param_str_opt(&params, "api_key")?;
    let default_model = param_str(&params, "default_model")?;
    let enabled = param_bool_opt(&params, "enabled")?.unwrap_or(true);
    let vps_url = param_str_opt(&params, "vps_url")?;

    let now = now_secs();
    let provider = UserProvider {
        id: Uuid::new_v4().to_string(),
        name,
        kind,
        base_url,
        api_key,
        default_model,
        enabled,
        created_at: now,
        updated_at: now,
        vps_url,
    };

    store::with_connection(&config, |conn| {
        store::provider_upsert(conn, &provider)
            .map_err(|e| anyhow::anyhow!("provider_upsert: {e}"))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(provider_id = %provider.id, "[ai_os][rpc] provider_add: ok");
    serde_json::to_value(&provider).map_err(|e| e.to_string())
}

pub async fn handle_provider_update(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] provider_update");
    let config = crate::openhuman::config::load_config_with_timeout().await?;

    let id = param_str(&params, "id")?;

    let mut provider = store::with_connection(&config, |conn| {
        store::provider_get(conn, &id)
            .map_err(|e| anyhow::anyhow!("provider_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("provider not found: {id}"))
    })
    .map_err(|e| e.to_string())?;

    if let Some(name) = param_str_opt(&params, "name")? {
        provider.name = name;
    }
    if let Some(kind_str) = param_str_opt(&params, "kind")? {
        provider.kind = parse_provider_kind(&kind_str)?;
    }
    if let Some(base_url) = param_str_opt(&params, "base_url")? {
        provider.base_url = base_url;
    }
    if let Some(api_key) = param_str_opt(&params, "api_key")? {
        provider.api_key = Some(api_key);
    }
    if let Some(default_model) = param_str_opt(&params, "default_model")? {
        provider.default_model = default_model;
    }
    if let Some(enabled) = param_bool_opt(&params, "enabled")? {
        provider.enabled = enabled;
    }
    if let Some(vps_url) = param_str_opt(&params, "vps_url")? {
        provider.vps_url = Some(vps_url);
    }
    provider.updated_at = now_secs();

    store::with_connection(&config, |conn| {
        store::provider_upsert(conn, &provider)
            .map_err(|e| anyhow::anyhow!("provider_upsert: {e}"))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(provider_id = %provider.id, "[ai_os][rpc] provider_update: ok");
    serde_json::to_value(&provider).map_err(|e| e.to_string())
}

pub async fn handle_provider_delete(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] provider_delete");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let id = param_str(&params, "id")?;

    store::with_connection(&config, |conn| {
        store::provider_delete(conn, &id).map_err(|e| anyhow::anyhow!("provider_delete: {e}"))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(provider_id = %id, "[ai_os][rpc] provider_delete: ok");
    Ok(serde_json::json!({ "ok": true }))
}

pub async fn handle_provider_list(_params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] provider_list");
    let config = crate::openhuman::config::load_config_with_timeout().await?;

    let providers = store::with_connection(&config, |conn| {
        store::provider_list(conn).map_err(|e| anyhow::anyhow!("provider_list: {e}"))
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(count = providers.len(), "[ai_os][rpc] provider_list: ok");
    let val = serde_json::to_value(&providers).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "providers": val }))
}

pub async fn handle_provider_test(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] provider_test");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let id = param_str(&params, "id")?;

    let provider = store::with_connection(&config, |conn| {
        store::provider_get(conn, &id)
            .map_err(|e| anyhow::anyhow!("provider_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("provider not found: {id}"))
    })
    .map_err(|e| e.to_string())?;

    let model = provider.default_model.clone();
    let start = std::time::Instant::now();
    let messages = vec![ChatMessage::user("Say hello")];

    match super::chat::chat(&provider, messages, &model).await {
        Ok(_) => {
            let latency_ms = start.elapsed().as_millis() as i64;
            tracing::debug!(
                provider_id = %id,
                latency_ms,
                "[ai_os][rpc] provider_test: ok"
            );
            Ok(serde_json::json!({ "ok": true, "latency_ms": latency_ms }))
        }
        Err(err) => {
            tracing::error!(
                provider_id = %id,
                error = %err,
                "[ai_os][rpc] provider_test: failed"
            );
            Ok(serde_json::json!({ "ok": false, "error": err }))
        }
    }
}

// ─── conversation handlers ────────────────────────────────────────────────────

pub async fn handle_conversation_create(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] conversation_create");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let provider_id = param_str(&params, "provider_id")?;

    let provider = store::with_connection(&config, |conn| {
        store::provider_get(conn, &provider_id)
            .map_err(|e| anyhow::anyhow!("provider_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("provider not found: {provider_id}"))
    })
    .map_err(|e| e.to_string())?;

    let model = param_str_opt(&params, "model")?.unwrap_or_else(|| provider.default_model.clone());
    let title = param_str_opt(&params, "title")?.unwrap_or_else(|| "New conversation".to_string());

    let now = now_secs();
    let conv = AiConversation {
        id: Uuid::new_v4().to_string(),
        title,
        provider_id,
        model,
        created_at: now,
        updated_at: now,
    };

    store::with_connection(&config, |conn| {
        store::conversation_upsert(conn, &conv)
            .map_err(|e| anyhow::anyhow!("conversation_upsert: {e}"))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(conversation_id = %conv.id, "[ai_os][rpc] conversation_create: ok");
    serde_json::to_value(&conv).map_err(|e| e.to_string())
}

pub async fn handle_conversation_list(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] conversation_list");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let provider_id = param_str_opt(&params, "provider_id")?;

    let conversations = store::with_connection(&config, |conn| {
        store::conversation_list(conn, provider_id.as_deref())
            .map_err(|e| anyhow::anyhow!("conversation_list: {e}"))
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(
        count = conversations.len(),
        "[ai_os][rpc] conversation_list: ok"
    );
    let val = serde_json::to_value(&conversations).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "conversations": val }))
}

pub async fn handle_conversation_get(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] conversation_get");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let id = param_str(&params, "id")?;

    let (conv, messages) = store::with_connection(&config, |conn| {
        let conv = store::conversation_get(conn, &id)
            .map_err(|e| anyhow::anyhow!("conversation_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("conversation not found: {id}"))?;
        let messages = store::messages_for_conversation(conn, &id)
            .map_err(|e| anyhow::anyhow!("messages_for_conversation: {e}"))?;
        Ok((conv, messages))
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(
        conversation_id = %id,
        message_count = messages.len(),
        "[ai_os][rpc] conversation_get: ok"
    );
    let conv_val = serde_json::to_value(&conv).map_err(|e| e.to_string())?;
    let msg_val = serde_json::to_value(&messages).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "conversation": conv_val, "messages": msg_val }))
}

pub async fn handle_conversation_delete(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] conversation_delete");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let id = param_str(&params, "id")?;

    store::with_connection(&config, |conn| {
        store::conversation_delete(conn, &id)
            .map_err(|e| anyhow::anyhow!("conversation_delete: {e}"))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(conversation_id = %id, "[ai_os][rpc] conversation_delete: ok");
    Ok(serde_json::json!({ "ok": true }))
}

// ─── chat handler ─────────────────────────────────────────────────────────────

pub async fn handle_chat_send(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] chat_send");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let conversation_id = param_str(&params, "conversation_id")?;
    let content = param_str(&params, "content")?;

    let (conv, provider) = store::with_connection(&config, |conn| {
        let conv = store::conversation_get(conn, &conversation_id)
            .map_err(|e| anyhow::anyhow!("conversation_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("conversation not found: {conversation_id}"))?;
        let provider = store::provider_get(conn, &conv.provider_id)
            .map_err(|e| anyhow::anyhow!("provider_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("provider not found: {}", conv.provider_id))?;
        Ok((conv, provider))
    })
    .map_err(|e| e.to_string())?;

    // Build history to send
    let history = store::with_connection(&config, |conn| {
        store::messages_for_conversation(conn, &conversation_id)
            .map_err(|e| anyhow::anyhow!("messages_for_conversation: {e}"))
    })
    .map_err(|e| e.to_string())?;

    let mut messages: Vec<ChatMessage> = history
        .iter()
        .map(|m| ChatMessage {
            id: Some(m.id.clone()),
            role: m.role.clone(),
            content: m.content.clone(),
            extra_metadata: None,
        })
        .collect();

    // Append the new user message
    let user_msg_id = Uuid::new_v4().to_string();
    messages.push(ChatMessage {
        id: Some(user_msg_id.clone()),
        role: "user".into(),
        content: content.clone(),
        extra_metadata: None,
    });

    // Dispatch
    let model = conv.model.clone();
    let (assistant_text, usage) = super::chat::chat(&provider, messages, &model).await?;

    let now = now_secs();

    // Persist user message
    let user_ai_msg = AiMessage {
        id: user_msg_id,
        conversation_id: conversation_id.clone(),
        role: "user".into(),
        content,
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
        created_at: now,
    };

    // Persist assistant message
    let assistant_ai_msg = AiMessage {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        role: "assistant".into(),
        content: assistant_text,
        input_tokens: usage.input_tokens as i64,
        output_tokens: usage.output_tokens as i64,
        cost_usd: usage.charged_amount_usd,
        created_at: now + 1, // ensure ordering
    };

    let assistant_ai_msg_clone = assistant_ai_msg.clone();
    store::with_connection(&config, |conn| {
        store::message_insert(conn, &user_ai_msg)
            .map_err(|e| anyhow::anyhow!("message_insert (user): {e}"))?;
        store::message_insert(conn, &assistant_ai_msg_clone)
            .map_err(|e| anyhow::anyhow!("message_insert (assistant): {e}"))?;
        // bump conversation updated_at
        conn.execute(
            "UPDATE ai_os_conversations SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, conversation_id],
        )
        .map_err(|e| anyhow::anyhow!("update conversation updated_at: {e}"))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(
        conversation_id = %conversation_id,
        input_tokens = usage.input_tokens,
        output_tokens = usage.output_tokens,
        "[ai_os][rpc] chat_send: ok"
    );

    let msg_val = serde_json::to_value(&assistant_ai_msg).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "message": msg_val,
        "usage": {
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "charged_amount_usd": usage.charged_amount_usd,
        }
    }))
}

// ─── conversation search handler ─────────────────────────────────────────────

pub async fn handle_conversation_search(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] conversation_search");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let query = param_str(&params, "query")?;
    let limit = param_i64_opt(&params, "limit")?.unwrap_or(20) as usize;

    let conversations = store::with_connection(&config, |conn| {
        store::conversation_search(conn, &query, limit)
            .map_err(|e| anyhow::anyhow!("conversation_search: {e}"))
    })
    .map_err(|e| e.to_string())?;

    tracing::debug!(
        query = %query,
        count = conversations.len(),
        "[ai_os][rpc] conversation_search: ok"
    );
    let val = serde_json::to_value(&conversations).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "conversations": val }))
}

// ─── models list handler ──────────────────────────────────────────────────────

pub async fn handle_models_list(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] models_list");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let provider_id = param_str(&params, "provider_id")?;

    let mut provider = store::with_connection(&config, |conn| {
        store::provider_get(conn, &provider_id)
            .map_err(|e| anyhow::anyhow!("provider_get: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("provider not found: {provider_id}"))
    })
    .map_err(|e| e.to_string())?;

    // Decrypt api_key if present
    if let Some(ref enc_key) = provider.api_key.clone() {
        if !enc_key.is_empty() {
            match super::key::decrypt_api_key(enc_key) {
                Ok(plain) => provider.api_key = Some(plain),
                Err(e) => {
                    tracing::debug!(
                        provider_id = %provider_id,
                        error = %e,
                        "[ai_os][rpc] models_list: failed to decrypt api_key, using as-is"
                    );
                }
            }
        }
    }

    let models = super::models::list_models(&provider).await;

    tracing::debug!(
        provider_id = %provider_id,
        count = models.len(),
        "[ai_os][rpc] models_list: ok"
    );
    let val = serde_json::to_value(&models).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "models": val, "provider_id": provider_id }))
}

// ─── usage handler ────────────────────────────────────────────────────────────

pub async fn handle_usage_get(params: Map<String, Value>) -> Result<Value, String> {
    tracing::debug!("[ai_os][rpc] usage_get");
    let config = crate::openhuman::config::load_config_with_timeout().await?;
    let days = param_i64_opt(&params, "days")?.unwrap_or(30);

    let by_provider = store::with_connection(&config, |conn| {
        store::usage_summary(conn, days).map_err(|e| anyhow::anyhow!("usage_summary: {e}"))
    })
    .map_err(|e| e.to_string())?;

    let total_input = by_provider.iter().map(|p| p.total_input_tokens).sum();
    let total_output = by_provider.iter().map(|p| p.total_output_tokens).sum();
    let total_cost = by_provider.iter().map(|p| p.total_cost_usd).sum();

    let summary = UsageSummary {
        by_provider,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cost_usd: total_cost,
        period_days: days,
    };

    tracing::debug!(period_days = days, "[ai_os][rpc] usage_get: ok");
    serde_json::to_value(&summary).map_err(|e| e.to_string())
}
