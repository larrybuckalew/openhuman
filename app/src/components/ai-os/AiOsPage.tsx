import { type FC, useEffect, useState } from 'react';

import {
  fetchConversation,
  fetchConversations,
  fetchProviders,
  fetchUsage,
  selectActiveConversation,
  selectProviders,
  selectStreamingContent,
  selectStreamingConversationId,
  selectUsage,
  streamMessage,
} from '../../store/aiOsSlice';
import { useAppDispatch, useAppSelector } from '../../store/hooks';
import type { UserProvider } from '../../types/aiOs';
import { AddProviderModal } from './AddProviderModal';
import { ChatInput } from './ChatInput';
import { ChatMessages } from './ChatMessages';
import { ProviderSidebar } from './ProviderSidebar';
import { TokenUsagePanel } from './TokenUsagePanel';

export const AiOsPage: FC = () => {
  const dispatch = useAppDispatch();

  const providers = useAppSelector(selectProviders);
  const conversations = useAppSelector(s => s.aiOs.conversations);
  const activeConversationId = useAppSelector(s => s.aiOs.activeConversationId);
  const activeConversation = useAppSelector(selectActiveConversation);
  const sendingMessage = useAppSelector(s => s.aiOs.sendingMessage);
  const streamingContent = useAppSelector(selectStreamingContent);
  const streamingConversationId = useAppSelector(selectStreamingConversationId);
  const usage = useAppSelector(selectUsage);

  const messages = useAppSelector(s =>
    activeConversationId ? (s.aiOs.messages[activeConversationId] ?? []) : []
  );

  const [showUsage, setShowUsage] = useState(false);
  const [showAddProvider, setShowAddProvider] = useState(false);
  const [editingProvider, setEditingProvider] = useState<UserProvider | null>(null);

  useEffect(() => {
    void dispatch(fetchProviders());
    void dispatch(fetchConversations());
    void dispatch(fetchUsage(30));
  }, [dispatch]);

  useEffect(() => {
    if (activeConversationId) {
      void dispatch(fetchConversation(activeConversationId));
    }
  }, [dispatch, activeConversationId]);

  const activeProvider = activeConversation
    ? (providers.find(p => p.id === activeConversation.provider_id) ?? null)
    : null;

  const handleSend = (content: string) => {
    if (!activeConversationId) return;
    void dispatch(streamMessage({ conversation_id: activeConversationId, content }));
  };

  const handleEditProvider = (provider: UserProvider) => {
    setEditingProvider(provider);
    setShowAddProvider(true);
  };

  const handleCloseModal = () => {
    setShowAddProvider(false);
    setEditingProvider(null);
  };

  return (
    <div className="flex flex-col h-full bg-white">
      <div className="flex items-center justify-between px-4 py-3 border-b border-stone-200 bg-white flex-shrink-0">
        <div className="flex items-center gap-2">
          <svg
            className="w-5 h-5 text-primary-500"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.8}
              d="M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 014.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15M14.25 3.104c.251.023.501.05.75.082M19.8 15a2.25 2.25 0 01.659 1.591v1.659a2.25 2.25 0 01-2.25 2.25H5.75a2.25 2.25 0 01-2.25-2.25v-1.659c0-.597.237-1.17.659-1.591L5 14.5m14.8.5l-3.197-3.197a4.5 4.5 0 00-6.361-.126M5 14.5l3.197-3.19a4.5 4.5 0 016.233-.203"
            />
          </svg>
          <span className="text-sm font-semibold text-stone-900">AI OS</span>
        </div>

        <button
          onClick={() => setShowUsage(v => !v)}
          className={`text-xs px-3 py-1.5 rounded-lg font-medium transition-colors ${
            showUsage
              ? 'bg-primary-100 text-primary-700'
              : 'bg-stone-100 text-stone-600 hover:bg-stone-200'
          }`}>
          Usage {showUsage ? '▲' : '▾'}
        </button>
      </div>

      <div className="flex flex-1 overflow-hidden">
        <ProviderSidebar
          providers={providers}
          conversations={conversations}
          activeConversationId={activeConversationId}
          onAddProvider={() => setShowAddProvider(true)}
          onEditProvider={handleEditProvider}
        />

        <div className="flex-1 flex overflow-hidden">
          {showUsage ? (
            <TokenUsagePanel usage={usage} />
          ) : activeConversation ? (
            <div className="flex-1 flex flex-col overflow-hidden">
              <ChatMessages
                messages={messages}
                sendingMessage={sendingMessage}
                streamingContent={streamingContent}
                streamingConversationId={streamingConversationId}
                activeConversationId={activeConversationId}
              />
              <ChatInput
                conversation={activeConversation}
                provider={activeProvider}
                sendingMessage={sendingMessage}
                onSend={handleSend}
              />
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center max-w-xs px-6">
                <div className="w-12 h-12 rounded-2xl bg-primary-50 flex items-center justify-center mx-auto mb-3">
                  <svg
                    className="w-6 h-6 text-primary-400"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.6}
                      d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
                    />
                  </svg>
                </div>
                <p className="text-sm text-stone-600 font-medium mb-1">No conversation selected</p>
                <p className="text-xs text-stone-400">
                  Select a provider and start a conversation, or click "+ New chat" under any
                  provider.
                </p>
              </div>
            </div>
          )}
        </div>
      </div>

      {showAddProvider && (
        <AddProviderModal onClose={handleCloseModal} editingProvider={editingProvider} />
      )}
    </div>
  );
};
