import { type FC, useEffect, useRef, useState } from 'react';

import {
  clearSearch,
  createConversation,
  searchConversations,
  selectSearchQuery,
  selectSearchResults,
  setActiveConversation,
} from '../../store/aiOsSlice';
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
  const searchResults = useAppSelector(selectSearchResults);
  const searchQuery = useAppSelector(selectSearchQuery);

  // Local input value — kept in sync with redux searchQuery, drives debounced dispatch
  const [searchInput, setSearchInput] = useState(searchQuery);
  const searchDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Debounced search dispatch
  useEffect(() => {
    if (searchDebounceRef.current) {
      clearTimeout(searchDebounceRef.current);
    }
    if (!searchInput.trim()) {
      dispatch(clearSearch());
      return;
    }
    searchDebounceRef.current = setTimeout(() => {
      void dispatch(searchConversations({ query: searchInput.trim() }));
    }, 300);
    return () => {
      if (searchDebounceRef.current) {
        clearTimeout(searchDebounceRef.current);
      }
    };
  }, [dispatch, searchInput]);

  const handleClearSearch = () => {
    setSearchInput('');
    dispatch(clearSearch());
  };

  // Tracks active endpoint preference per provider: 'local' | 'vps'
  const [endpointPrefs, setEndpointPrefs] = useState<Map<string, 'local' | 'vps'>>(new Map());

  const toggleEndpoint = (providerId: string) => {
    setEndpointPrefs(prev => {
      const next = new Map(prev);
      next.set(providerId, prev.get(providerId) === 'vps' ? 'local' : 'vps');
      return next;
    });
  };

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
              {providers.map(p => {
                const activeEndpoint = endpointPrefs.get(p.id) ?? 'local';
                return (
                  <div key={p.id}>
                    <ProviderCard provider={p} onEdit={onEditProvider} />
                    {p.vps_url && (
                      <div className="mt-1 flex items-center gap-1">
                        <button
                          onClick={() => toggleEndpoint(p.id)}
                          className={`flex items-center rounded-full border text-[10px] font-medium overflow-hidden transition-colors ${
                            activeEndpoint === 'local' ? 'border-stone-300' : 'border-primary-400'
                          }`}
                          title={`Switch to ${activeEndpoint === 'local' ? 'VPS' : 'Local'} endpoint`}>
                          <span
                            className={`px-2 py-0.5 transition-colors ${
                              activeEndpoint === 'local'
                                ? 'bg-stone-200 text-stone-700'
                                : 'bg-white text-stone-400'
                            }`}>
                            Local
                          </span>
                          <span
                            className={`px-2 py-0.5 transition-colors ${
                              activeEndpoint === 'vps'
                                ? 'bg-primary-500 text-white'
                                : 'bg-white text-stone-400'
                            }`}>
                            VPS
                          </span>
                        </button>
                      </div>
                    )}
                    <button
                      onClick={() => handleNewConversation(p)}
                      disabled={sendingMessage}
                      className="mt-1 w-full text-[11px] py-1 rounded-md bg-stone-100 hover:bg-primary-50 text-stone-500 hover:text-primary-600 transition-colors disabled:opacity-50">
                      + New chat
                    </button>
                  </div>
                );
              })}
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

            {/* Search input */}
            <div className="relative mb-2">
              <span className="absolute left-2 top-1/2 -translate-y-1/2 pointer-events-none text-stone-400">
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M21 21l-4.35-4.35M17 11A6 6 0 1 0 5 11a6 6 0 0 0 12 0z"
                  />
                </svg>
              </span>
              <input
                type="text"
                value={searchInput}
                onChange={e => setSearchInput(e.target.value)}
                placeholder="Search conversations…"
                className="w-full text-[11px] pl-6 pr-6 py-1.5 rounded-lg border border-stone-200 bg-white text-stone-700 placeholder-stone-400 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent"
              />
              {searchInput && (
                <button
                  onClick={handleClearSearch}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-stone-400 hover:text-stone-600 transition-colors"
                  aria-label="Clear search">
                  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </button>
              )}
            </div>

            {/* Search results */}
            {searchResults !== null ? (
              <div className="space-y-0.5">
                {searchResults.length === 0 ? (
                  <p className="text-[11px] text-stone-400 text-center py-3">
                    No conversations found
                  </p>
                ) : (
                  searchResults.map(c => {
                    const providerName =
                      providers.find(p => p.id === c.provider_id)?.name ?? c.provider_id;
                    return (
                      <button
                        key={c.id}
                        onClick={() => dispatch(setActiveConversation(c.id))}
                        className={`w-full text-left px-2 py-1.5 rounded-lg transition-colors ${
                          activeConversationId === c.id
                            ? 'bg-primary-100 text-primary-800 font-medium'
                            : 'text-stone-600 hover:bg-stone-200'
                        }`}>
                        <div className="text-xs truncate">{c.title}</div>
                        <div className="text-[10px] text-stone-400 truncate">{providerName}</div>
                      </button>
                    );
                  })
                )}
              </div>
            ) : (
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
            )}
          </div>
        )}
      </div>
    </div>
  );
};
