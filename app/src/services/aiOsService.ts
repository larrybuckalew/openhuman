import debug from 'debug';

import type { AiConversation, AiMessage, UsageSummary, UserProvider } from '../types/aiOs';
import { callCoreRpc } from './coreRpcClient';

const log = debug('ai-os:service');

export interface AddProviderParams {
  name: string;
  kind: string;
  base_url: string;
  api_key?: string;
  default_model: string;
  enabled: boolean;
}

export interface UpdateProviderParams {
  id: string;
  name?: string;
  kind?: string;
  base_url?: string;
  api_key?: string;
  default_model?: string;
  enabled?: boolean;
}

export interface ProviderTestResult {
  ok: boolean;
  latency_ms?: number;
  error?: string;
}

export interface CreateConversationParams {
  provider_id: string;
  model?: string;
  title?: string;
}

export interface ConversationDetail {
  conversation: AiConversation;
  messages: AiMessage[];
}

export interface SendMessageResult {
  message: AiMessage;
  usage: {
    input_tokens: number;
    output_tokens: number;
    cost_usd: number;
  };
}

export async function listProviders(): Promise<UserProvider[]> {
  log('listProviders');
  const result = await callCoreRpc<{ providers: UserProvider[] }>({
    method: 'openhuman.ai_os_provider_list',
    params: {},
  });
  return result.providers;
}

export async function addProvider(params: AddProviderParams): Promise<UserProvider> {
  log('addProvider name=%s kind=%s', params.name, params.kind);
  return callCoreRpc<UserProvider>({
    method: 'openhuman.ai_os_provider_add',
    params,
  });
}

export async function updateProvider(params: UpdateProviderParams): Promise<UserProvider> {
  log('updateProvider id=%s', params.id);
  return callCoreRpc<UserProvider>({
    method: 'openhuman.ai_os_provider_update',
    params,
  });
}

export async function deleteProvider(id: string): Promise<void> {
  log('deleteProvider id=%s', id);
  await callCoreRpc<{ ok: true }>({
    method: 'openhuman.ai_os_provider_delete',
    params: { id },
  });
}

export async function testProvider(id: string): Promise<ProviderTestResult> {
  log('testProvider id=%s', id);
  return callCoreRpc<ProviderTestResult>({
    method: 'openhuman.ai_os_provider_test',
    params: { id },
  });
}

export async function createConversation(params: CreateConversationParams): Promise<AiConversation> {
  log('createConversation provider_id=%s', params.provider_id);
  return callCoreRpc<AiConversation>({
    method: 'openhuman.ai_os_conversation_create',
    params,
  });
}

export async function listConversations(provider_id?: string): Promise<AiConversation[]> {
  log('listConversations provider_id=%s', provider_id ?? 'all');
  const result = await callCoreRpc<{ conversations: AiConversation[] }>({
    method: 'openhuman.ai_os_conversation_list',
    params: provider_id ? { provider_id } : {},
  });
  return result.conversations;
}

export async function getConversation(id: string): Promise<ConversationDetail> {
  log('getConversation id=%s', id);
  return callCoreRpc<ConversationDetail>({
    method: 'openhuman.ai_os_conversation_get',
    params: { id },
  });
}

export async function deleteConversation(id: string): Promise<void> {
  log('deleteConversation id=%s', id);
  await callCoreRpc<{ ok: true }>({
    method: 'openhuman.ai_os_conversation_delete',
    params: { id },
  });
}

export async function sendMessage(
  conversation_id: string,
  content: string
): Promise<SendMessageResult> {
  log('sendMessage conversation_id=%s', conversation_id);
  return callCoreRpc<SendMessageResult>({
    method: 'openhuman.ai_os_chat_send',
    params: { conversation_id, content },
  });
}

export async function getUsage(days?: number): Promise<UsageSummary> {
  log('getUsage days=%d', days ?? 30);
  return callCoreRpc<UsageSummary>({
    method: 'openhuman.ai_os_usage_get',
    params: days !== undefined ? { days } : {},
  });
}
