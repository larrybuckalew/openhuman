import { type FC, type KeyboardEvent, useRef, useState } from 'react';

import type { AiConversation, UserProvider } from '../../types/aiOs';

interface ChatInputProps {
  conversation: AiConversation;
  provider: UserProvider | null;
  sendingMessage: boolean;
  onSend: (content: string) => void;
}

export const ChatInput: FC<ChatInputProps> = ({ conversation, provider, sendingMessage, onSend }) => {
  const [content, setContent] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleSend = () => {
    const trimmed = content.trim();
    if (!trimmed || sendingMessage) return;
    onSend(trimmed);
    setContent('');
    textareaRef.current?.focus();
  };

  return (
    <div className="border-t border-stone-200 bg-white px-4 py-3">
      {provider && (
        <div className="flex items-center gap-2 mb-2">
          <span className="inline-flex items-center gap-1.5 text-[11px] px-2.5 py-1 rounded-full bg-stone-100 text-stone-500 border border-stone-200">
            <span className="font-medium text-stone-600">{provider.name}</span>
            <span className="text-stone-400">·</span>
            <span className="font-mono">{conversation.model}</span>
          </span>
        </div>
      )}

      <div className="flex items-end gap-2">
        <textarea
          ref={textareaRef}
          value={content}
          onChange={e => setContent(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Message… (Enter to send, Shift+Enter for new line)"
          rows={1}
          disabled={sendingMessage}
          className="flex-1 resize-none text-sm px-3 py-2.5 rounded-xl border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent placeholder-stone-400 disabled:opacity-50 leading-relaxed overflow-hidden"
          style={{ minHeight: '40px', maxHeight: '160px' }}
          onInput={e => {
            const el = e.currentTarget;
            el.style.height = 'auto';
            el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
          }}
        />
        <button
          onClick={handleSend}
          disabled={!content.trim() || sendingMessage}
          className="flex-shrink-0 w-9 h-9 flex items-center justify-center rounded-xl bg-primary-500 hover:bg-primary-600 text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed">
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"
            />
          </svg>
        </button>
      </div>
    </div>
  );
};
