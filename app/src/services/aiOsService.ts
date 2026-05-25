import debug from 'debug';

import type {
  AiConversation,
  AiMessage,
  ModelInfo,
  UsageSummary,
  UserProvider,
} from '../types/aiOs';
import { callCoreRpc, getCoreHttpBaseUrl, getCoreRpcToken } from './coreRpcClient';

const log = debug('ai-os:service');

export interface AddProviderParams {
  name: string;
  kind: string;
  base_url: string;
  api_key?: string;
  default_model: string;
  enabled: boolean;
  vps_url?: string;
}

export interface UpdateProviderParams {
  id: string;
  name?: string;
  kind?: string;
  base_url?: string;
  api_key?: string;
  default_model?: string;
  enabled?: boolean;
  vps_url?: string;
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
  usage: { input_tokens: number; output_tokens: number; cost_usd: number };
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
  return callCoreRpc<UserProvider>({ method: 'openhuman.ai_os_provider_add', params });
}

export async function updateProvider(params: UpdateProviderParams): Promise<UserProvider> {
  log('updateProvider id=%s', params.id);
  return callCoreRpc<UserProvider>({ method: 'openhuman.ai_os_provider_update', params });
}

export async function deleteProvider(id: string): Promise<void> {
  log('deleteProvider id=%s', id);
  await callCoreRpc<{ ok: true }>({ method: 'openhuman.ai_os_provider_delete', params: { id } });
}

export async function testProvider(id: string): Promise<ProviderTestResult> {
  log('testProvider id=%s', id);
  return callCoreRpc<ProviderTestResult>({
    method: 'openhuman.ai_os_provider_test',
    params: { id },
  });
}

export async function createConversation(
  params: CreateConversationParams
): Promise<AiConversation> {
  log('createConversation provider_id=%s', params.provider_id);
  return callCoreRpc<AiConversation>({ method: 'openhuman.ai_os_conversation_create', params });
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

export async function listModels(
  providerId: string
): Promise<{ models: ModelInfo[]; provider_id: string }> {
  log('listModels providerId=%s', providerId);
  return callCoreRpc<{ models: ModelInfo[]; provider_id: string }>({
    method: 'openhuman.ai_os_models_list',
    params: { provider_id: providerId },
  });
}

export async function searchConversations(
  query: string,
  limit?: number
): Promise<{ conversations: AiConversation[] }> {
  log('searchConversations query=%s limit=%d', query, limit ?? 20);
  return callCoreRpc<{ conversations: AiConversation[] }>({
    method: 'openhuman.ai_os_conversation_search',
    params: limit !== undefined ? { query, limit } : { query },
  });
}

export async function getUsage(days?: number): Promise<UsageSummary> {
  log('getUsage days=%d', days ?? 30);
  return callCoreRpc<UsageSummary>({
    method: 'openhuman.ai_os_usage_get',
    params: days !== undefined ? { days } : {},
  });
}

/**
 * Stream a chat message via the SSE endpoint `POST /ai-os/stream`.
 *
 * Uses `fetch` (not `EventSource`) because EventSource does not support POST
 * with a request body. The response body is consumed as a ReadableStream and
 * parsed as Server-Sent Events manually.
 *
 * SSE protocol used by the core:
 *   `event: delta\ndata: <text chunk>\n\n`  — streaming text token
 *   `event: done\ndata: {"input_tokens":N,"output_tokens":N,"cost_usd":F}\n\n`  — final usage
 */
export async function streamChat(
  conversationId: string,
  content: string,
  onDelta: (text: string) => void,
  onDone: (usage: { input_tokens: number; output_tokens: number; cost_usd: number }) => void,
  onError: (err: string) => void,
  model?: string
): Promise<void> {
  log('streamChat conversation_id=%s model=%s', conversationId, model ?? 'default');

  let baseUrl: string;
  let token: string | null;
  try {
    [baseUrl, token] = await Promise.all([getCoreHttpBaseUrl(), getCoreRpcToken()]);
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Failed to resolve core RPC URL/token';
    log('streamChat resolve error: %s', msg);
    onError(msg);
    return;
  }

  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  let response: Response;
  try {
    response = await fetch(`${baseUrl}/ai-os/stream`, {
      method: 'POST',
      headers,
      body: JSON.stringify(
        model
          ? { conversation_id: conversationId, content, model }
          : { conversation_id: conversationId, content }
      ),
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Network error on stream request';
    log('streamChat fetch error: %s', msg);
    onError(msg);
    return;
  }

  if (!response.ok) {
    const text = await response.text().catch(() => response.statusText);
    const msg = `Stream request failed (HTTP ${response.status}): ${text}`;
    log('streamChat HTTP error: %s', msg);
    onError(msg);
    return;
  }

  if (!response.body) {
    onError('Stream response has no body');
    return;
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  const processEvent = (rawBlock: string) => {
    // Each SSE block is a set of lines; extract `event:` and `data:` values.
    // Per SSE spec, strip exactly one leading space from data values.
    // Multiple data: lines are concatenated with newlines.
    let eventName = '';
    let dataLine: string | null = null;
    for (const line of rawBlock.split('\n')) {
      if (line.startsWith('event:')) {
        eventName = line.slice('event:'.length).trim();
      } else if (line.startsWith('data:')) {
        // Strip exactly one leading space per SSE spec.
        const value = line.startsWith('data: ') ? line.slice(6) : line.slice(5);
        dataLine = dataLine !== null ? dataLine + '\n' + value : value;
      }
    }
    if (!eventName || dataLine === null) return;

    if (eventName === 'error') {
      onError(dataLine ?? 'Stream error');
      return;
    }

    if (eventName === 'delta') {
      onDelta(dataLine);
    } else if (eventName === 'done') {
      try {
        const usage = JSON.parse(dataLine) as {
          input_tokens: number;
          output_tokens: number;
          cost_usd: number;
        };
        onDone(usage);
      } catch {
        log('streamChat failed to parse done payload: %s', dataLine);
      }
    }

    // Reset for next event.
    eventName = '';
    dataLine = null;
  };

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      // SSE events are separated by double newline.
      const parts = buffer.split('\n\n');
      // The last part may be an incomplete event — keep it in the buffer.
      buffer = parts.pop() ?? '';
      for (const part of parts) {
        const trimmed = part.trim();
        if (trimmed) processEvent(trimmed);
      }
    }
    // Flush any remaining content in the buffer.
    if (buffer.trim()) processEvent(buffer.trim());
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Stream read error';
    log('streamChat stream read error: %s', msg);
    onError(msg);
  } finally {
    reader.releaseLock();
  }
}
