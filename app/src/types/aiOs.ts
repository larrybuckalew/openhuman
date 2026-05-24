export type ProviderKind = 'openai_compatible' | 'anthropic' | 'google';

export interface UserProvider {
  id: string;
  name: string;
  kind: ProviderKind;
  base_url: string;
  api_key?: string;
  default_model: string;
  enabled: boolean;
  vps_url?: string;
  created_at: number;
  updated_at: number;
}

export interface AiConversation {
  id: string;
  title: string;
  provider_id: string;
  model: string;
  created_at: number;
  updated_at: number;
}

export interface AiMessage {
  id: string;
  conversation_id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  created_at: number;
}

export interface ProviderUsageSummary {
  provider_id: string;
  provider_name: string;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_usd: number;
  request_count: number;
}

export interface UsageSummary {
  by_provider: ProviderUsageSummary[];
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_usd: number;
  period_days: number;
}

export interface ProviderTemplate {
  name: string;
  kind: ProviderKind;
  base_url: string;
  default_model: string;
  description: string;
  requires_api_key: boolean;
}

export interface ModelInfo {
  id: string;
  name: string;
  context_length?: number;
  description?: string;
}
