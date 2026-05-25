use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderKind {
    // OpenAI, OpenRouter, Ollama, Hermès, Openclaw, Codex, Kilocode
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "google")]
    Google,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProvider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Optional secondary / VPS endpoint URL for Hermès/Openclaw dual-endpoint routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vps_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConversation {
    pub id: String,
    pub title: String,
    pub provider_id: String,
    pub model: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsageSummary {
    pub provider_id: String,
    pub provider_name: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_usd: f64,
    pub request_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub by_provider: Vec<ProviderUsageSummary>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_usd: f64,
    pub period_days: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_serializes_as_frontend_wire_format() {
        // Frontend `ProviderKind` TS type is the source of truth for these
        // tokens — keep these assertions stable to lock the contract.
        assert_eq!(
            serde_json::to_string(&ProviderKind::OpenAiCompatible).unwrap(),
            "\"openai_compatible\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderKind::Anthropic).unwrap(),
            "\"anthropic\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderKind::Google).unwrap(),
            "\"google\""
        );
    }

    #[test]
    fn provider_kind_deserializes_from_frontend_wire_format() {
        let k: ProviderKind = serde_json::from_str("\"openai_compatible\"").unwrap();
        assert_eq!(k, ProviderKind::OpenAiCompatible);
        let k: ProviderKind = serde_json::from_str("\"anthropic\"").unwrap();
        assert_eq!(k, ProviderKind::Anthropic);
        let k: ProviderKind = serde_json::from_str("\"google\"").unwrap();
        assert_eq!(k, ProviderKind::Google);
    }

    #[test]
    fn provider_kind_rejects_legacy_wire_token() {
        // The legacy snake_case form (auto-generated from
        // `rename_all = "snake_case"` for the `OpenAiCompatible` variant)
        // must NOT round-trip through serde anymore — the store-layer
        // migration is responsible for rewriting old DB rows.
        let r: Result<ProviderKind, _> = serde_json::from_str("\"open_ai_compatible\"");
        assert!(r.is_err());
    }
}
