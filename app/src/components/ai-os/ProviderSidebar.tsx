import { type FC } from 'react';

import { createConversation, setActiveConversation } from '../../store/aiOsSlice';
import { useAppDispatch, useAppSelector } from '../../store/hooks';
import type { AiConversation, UserProvider } from '../../types/aiOs';
import { ProviderCard } from './ProviderCard';

interface ProviderSidebarProps {
  providers: UserProvider[];
  conversations: AiConversation[];
  activeConversationId: string | null;
  onAddProvider: () => void;
  onEditProvider: (provider: UserProvider) => void;
}

export const ProviderSidebar: FC<ProviderSidebarProps> = ({
  providers,
  conversations,
  activeConversationId,
  onAddProvider,
  onEditProvider,
}) => {
  const dispatch = useAppDispatch();
  const sendingMessage = useAppSelector(s => s.aiOs.sendingMessage);

  const handleNewConversation = (provider: UserProvider) => {
    void dispatch(createConversation({ provider_id: provider.id, model: provider.default_model }));
  };

  const conversationsByProvider: Record<string, AiConversation[]> = {};
  for (const c of conversations) {
    if (!conversationsByProvider[c.provider_id]) {
      conversationsByProvider[c.provider_id] = [];
    }
    conversationsByProvider[c.provider_id].push(c);
  }

  return (
    <div className="w-60 flex-shrink-0 border-r border-stone-200 bg-stone-50 flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-3 py-3 space-y-4">
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-[10px] font-semibold text-stone-400 uppercase tracking-widest">
              Providers
            </span>
            <button
              onClick={onAddProvider}
              className="text-[11px] px-2 py-0.5 rounded-md bg-primary-50 hover:bg-primary-100 text-primary-600 font-medium transition-colors">
              + Add
            </button>
          </div>

          {providers.length === 0 ? (
            <div className="text-xs text-stone-400 text-center py-4 px-2">
              No providers yet. Add one to get started.
            </div>
          ) : (
            <div className="space-y-2">
              {providers.map(p => (
                <div key={p.id}>
                  <ProviderCard provider={p} onEdit={onEditProvider} />
                  <button
                    onClick={() => handleNewConversation(p)}
                    disabled={sendingMessage}
                    className="mt-1 w-full text-[11px] py-1 rounded-md bg-stone-100 hover:bg-primary-50 text-stone-500 hover:text-primary-600 transition-colors disabled:opacity-50">
                    + New chat
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        {conversations.length > 0 && (
          <div>
            <div className="mb-2">
              <span className="text-[10px] font-semibold text-stone-400 uppercase tracking-widest">
                Conversations
              </span>
            </div>

            <div className="space-y-3">
              {providers
                .filter(p => conversationsByProvider[p.id]?.length)
                .map(p => (
                  <div key={p.id}>
                    <div className="text-[10px] font-medium text-stone-400 mb-1 truncate">
                      {p.name}
                    </div>
                    <div className="space-y-0.5">
                      {(conversationsByProvider[p.id] ?? []).map(c => (
                        <button
                          key={c.id}
                          onClick={() => dispatch(setActiveConversation(c.id))}
                          className={`w-full text-left text-xs px-2 py-1.5 rounded-lg truncate transition-colors ${
                            activeConversationId === c.id
                              ? 'bg-primary-100 text-primary-800 font-medium'
                              : 'text-stone-600 hover:bg-stone-200'
                          }`}>
                          {c.title}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
