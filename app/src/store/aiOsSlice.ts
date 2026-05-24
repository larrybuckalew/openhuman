import { createAsyncThunk, createSlice, type PayloadAction } from '@reduxjs/toolkit';
import debug from 'debug';

import * as aiOsService from '../services/aiOsService';
import type { AddProviderParams, UpdateProviderParams } from '../services/aiOsService';
import type { AiConversation, AiMessage, UsageSummary, UserProvider } from '../types/aiOs';
import type { AppDispatch } from './index';
import { resetUserScopedState } from './resetActions';

const log = debug('ai-os:slice');

interface ProviderTestState {
  status: 'idle' | 'pending' | 'ok' | 'error';
  latency_ms?: number;
  error?: string;
}

interface AiOsState {
  providers: UserProvider[];
  conversations: AiConversation[];
  activeConversationId: string | null;
  messages: Record<string, AiMessage[]>;
  usage: UsageSummary | null;
  status: 'idle' | 'loading' | 'error';
  error: string | null;
  sendingMessage: boolean;
  providerTestStates: Record<string, ProviderTestState>;
  streamingContent: string;
  streamingConversationId: string | null;
}

const initialState: AiOsState = {
  providers: [],
  conversations: [],
  activeConversationId: null,
  messages: {},
  usage: null,
  status: 'idle',
  error: null,
  sendingMessage: false,
  providerTestStates: {},
  streamingContent: '',
  streamingConversationId: null,
};

export const fetchProviders = createAsyncThunk('aiOs/fetchProviders', async () => {
  log('fetchProviders');
  return aiOsService.listProviders();
});

export const addProvider = createAsyncThunk(
  'aiOs/addProvider',
  async (params: AddProviderParams) => {
    log('addProvider name=%s', params.name);
    return aiOsService.addProvider(params);
  }
);

export const updateProvider = createAsyncThunk(
  'aiOs/updateProvider',
  async (params: UpdateProviderParams) => {
    log('updateProvider id=%s', params.id);
    return aiOsService.updateProvider(params);
  }
);

export const deleteProvider = createAsyncThunk('aiOs/deleteProvider', async (id: string) => {
  log('deleteProvider id=%s', id);
  await aiOsService.deleteProvider(id);
  return id;
});

export const testProvider = createAsyncThunk('aiOs/testProvider', async (id: string) => {
  log('testProvider id=%s', id);
  const result = await aiOsService.testProvider(id);
  return { id, ...result };
});

export const createConversation = createAsyncThunk(
  'aiOs/createConversation',
  async (params: { provider_id: string; model?: string; title?: string }) => {
    log('createConversation provider_id=%s', params.provider_id);
    return aiOsService.createConversation(params);
  }
);

export const fetchConversations = createAsyncThunk(
  'aiOs/fetchConversations',
  async (provider_id?: string) => {
    log('fetchConversations provider_id=%s', provider_id ?? 'all');
    return aiOsService.listConversations(provider_id);
  }
);

export const fetchConversation = createAsyncThunk('aiOs/fetchConversation', async (id: string) => {
  log('fetchConversation id=%s', id);
  return aiOsService.getConversation(id);
});

export const deleteConversation = createAsyncThunk(
  'aiOs/deleteConversation',
  async (id: string) => {
    log('deleteConversation id=%s', id);
    await aiOsService.deleteConversation(id);
    return id;
  }
);

export const sendMessage = createAsyncThunk(
  'aiOs/sendMessage',
  async ({ conversation_id, content }: { conversation_id: string; content: string }) => {
    log('sendMessage conversation_id=%s', conversation_id);
    return aiOsService.sendMessage(conversation_id, content);
  }
);

/**
 * Stream a chat message via the SSE endpoint (`POST /ai-os/stream`).
 *
 * Dispatches `setStreamingChunk` for each arriving delta, then reloads the
 * conversation once the `done` event fires so the final persisted message
 * (with accurate token counts) replaces the streamed content in the store.
 */
export const streamMessage =
  ({ conversation_id, content }: { conversation_id: string; content: string }) =>
  async (dispatch: AppDispatch) => {
    log('streamMessage conversation_id=%s', conversation_id);
    dispatch(setStreamingChunk({ conversationId: conversation_id, chunk: '' }));
    dispatch(aiOsSlice.actions._setStreamingConversation(conversation_id));
    dispatch(aiOsSlice.actions._setSendingMessage(true));
    dispatch(clearError());

    await aiOsService.streamChat(
      conversation_id,
      content,
      chunk => {
        dispatch(setStreamingChunk({ conversationId: conversation_id, chunk }));
      },
      async _usage => {
        log('streamMessage done conversation_id=%s', conversation_id);
        await dispatch(fetchConversation(conversation_id));
        dispatch(clearStreaming());
        dispatch(aiOsSlice.actions._setSendingMessage(false));
      },
      err => {
        log('streamMessage error conversation_id=%s err=%s', conversation_id, err);
        dispatch(aiOsSlice.actions._setError(err));
        dispatch(clearStreaming());
        dispatch(aiOsSlice.actions._setSendingMessage(false));
      }
    );
  };

export const fetchUsage = createAsyncThunk('aiOs/fetchUsage', async (days?: number) => {
  log('fetchUsage days=%d', days ?? 30);
  return aiOsService.getUsage(days);
});

const aiOsSlice = createSlice({
  name: 'aiOs',
  initialState,
  reducers: {
    setActiveConversation(state, action: PayloadAction<string | null>) {
      state.activeConversationId = action.payload;
    },
    clearError(state) {
      state.error = null;
    },
    /**
     * Append a streaming text chunk for the given conversation.
     * When `chunk` is an empty string (first dispatch), it resets the buffer
     * and sets `streamingConversationId` so the UI knows streaming has begun.
     */
    setStreamingChunk(
      state,
      action: PayloadAction<{ conversationId: string; chunk: string }>
    ) {
      const { conversationId, chunk } = action.payload;
      if (!chunk) {
        // Empty chunk signals stream start — reset buffer and bind conversation.
        state.streamingContent = '';
        state.streamingConversationId = conversationId;
      } else {
        state.streamingContent += chunk;
      }
    },
    /** Reset streaming state after a stream completes or errors. */
    clearStreaming(state) {
      state.streamingContent = '';
      state.streamingConversationId = null;
    },
    // Internal helpers used by the `streamMessage` thunk —
    // not intended for direct dispatch from UI components.
    _setStreamingConversation(state, action: PayloadAction<string | null>) {
      state.streamingConversationId = action.payload;
    },
    _setSendingMessage(state, action: PayloadAction<boolean>) {
      state.sendingMessage = action.payload;
    },
    _setError(state, action: PayloadAction<string>) {
      state.error = action.payload;
    },
  },
  extraReducers: builder => {
    builder.addCase(resetUserScopedState, () => initialState);

    builder
      .addCase(fetchProviders.pending, state => {
        state.status = 'loading';
        state.error = null;
      })
      .addCase(fetchProviders.fulfilled, (state, action) => {
        state.status = 'idle';
        state.providers = action.payload;
      })
      .addCase(fetchProviders.rejected, (state, action) => {
        state.status = 'error';
        state.error = action.error.message ?? 'Failed to fetch providers';
      });

    builder
      .addCase(addProvider.fulfilled, (state, action) => {
        state.providers.push(action.payload);
      })
      .addCase(addProvider.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to add provider';
      });

    builder
      .addCase(updateProvider.fulfilled, (state, action) => {
        const idx = state.providers.findIndex(p => p.id === action.payload.id);
        if (idx !== -1) {
          state.providers[idx] = action.payload;
        }
      })
      .addCase(updateProvider.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to update provider';
      });

    builder
      .addCase(deleteProvider.fulfilled, (state, action) => {
        state.providers = state.providers.filter(p => p.id !== action.payload);
        state.conversations = state.conversations.filter(c => c.provider_id !== action.payload);
        if (
          state.activeConversationId &&
          !state.conversations.find(c => c.id === state.activeConversationId)
        ) {
          state.activeConversationId = null;
        }
      })
      .addCase(deleteProvider.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to delete provider';
      });

    builder
      .addCase(testProvider.pending, (state, action) => {
        state.providerTestStates[action.meta.arg] = { status: 'pending' };
      })
      .addCase(testProvider.fulfilled, (state, action) => {
        state.providerTestStates[action.payload.id] = {
          status: action.payload.ok ? 'ok' : 'error',
          latency_ms: action.payload.latency_ms,
          error: action.payload.error,
        };
      })
      .addCase(testProvider.rejected, (state, action) => {
        state.providerTestStates[action.meta.arg] = {
          status: 'error',
          error: action.error.message ?? 'Test failed',
        };
      });

    builder
      .addCase(createConversation.fulfilled, (state, action) => {
        state.conversations.unshift(action.payload);
        state.activeConversationId = action.payload.id;
        state.messages[action.payload.id] = [];
      })
      .addCase(createConversation.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to create conversation';
      });

    builder
      .addCase(fetchConversations.fulfilled, (state, action) => {
        state.conversations = action.payload;
      })
      .addCase(fetchConversations.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to fetch conversations';
      });

    builder
      .addCase(fetchConversation.fulfilled, (state, action) => {
        const { conversation, messages } = action.payload;
        const idx = state.conversations.findIndex(c => c.id === conversation.id);
        if (idx !== -1) {
          state.conversations[idx] = conversation;
        } else {
          state.conversations.unshift(conversation);
        }
        state.messages[conversation.id] = messages;
      })
      .addCase(fetchConversation.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to fetch conversation';
      });

    builder
      .addCase(deleteConversation.fulfilled, (state, action) => {
        state.conversations = state.conversations.filter(c => c.id !== action.payload);
        delete state.messages[action.payload];
        if (state.activeConversationId === action.payload) {
          state.activeConversationId = null;
        }
      })
      .addCase(deleteConversation.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to delete conversation';
      });

    builder
      .addCase(sendMessage.pending, state => {
        state.sendingMessage = true;
        state.error = null;
      })
      .addCase(sendMessage.fulfilled, (state, action) => {
        state.sendingMessage = false;
        const convId = action.payload.message.conversation_id;
        if (!state.messages[convId]) {
          state.messages[convId] = [];
        }
        state.messages[convId].push(action.payload.message);
      })
      .addCase(sendMessage.rejected, (state, action) => {
        state.sendingMessage = false;
        state.error = action.error.message ?? 'Failed to send message';
      });

    builder
      .addCase(fetchUsage.fulfilled, (state, action) => {
        state.usage = action.payload;
      })
      .addCase(fetchUsage.rejected, (state, action) => {
        state.error = action.error.message ?? 'Failed to fetch usage';
      });
  },
});

export const { setActiveConversation, clearError, setStreamingChunk, clearStreaming } =
  aiOsSlice.actions;

export const selectProviders = (state: { aiOs: AiOsState }) => state.aiOs.providers;

export const selectActiveConversation = (state: { aiOs: AiOsState }) => {
  const id = state.aiOs.activeConversationId;
  if (!id) return null;
  return state.aiOs.conversations.find(c => c.id === id) ?? null;
};

export const selectConversationMessages =
  (conversationId: string) => (state: { aiOs: AiOsState }) =>
    state.aiOs.messages[conversationId] ?? [];

export const selectUsage = (state: { aiOs: AiOsState }) => state.aiOs.usage;

export const selectProviderTestState = (providerId: string) => (state: { aiOs: AiOsState }) =>
  state.aiOs.providerTestStates[providerId] ?? { status: 'idle' };

export const selectStreamingContent = (state: { aiOs: AiOsState }) => state.aiOs.streamingContent;
export const selectStreamingConversationId = (state: { aiOs: AiOsState }) =>
  state.aiOs.streamingConversationId;

export default aiOsSlice.reducer;
