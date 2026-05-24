import { type FC, useEffect, useRef } from 'react';

import type { AiMessage } from '../../types/aiOs';

interface ChatMessagesProps {
  messages: AiMessage[];
  sendingMessage: boolean;
}

function formatCost(usd: number): string {
  if (usd < 0.0001) return '<$0.0001';
  return `$${usd.toFixed(4)}`;
}

export const ChatMessages: FC<ChatMessagesProps> = ({ messages, sendingMessage }) => {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages.length, sendingMessage]);

  if (messages.length === 0 && !sendingMessage) {
    return (
      <div className="flex-1 flex items-center justify-center text-stone-400 text-sm">
        Send a message to start the conversation.
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
      {messages.map(msg => (
        <div
          key={msg.id}
          className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
          <div
            className={`max-w-[75%] ${msg.role === 'user' ? 'items-end' : 'items-start'} flex flex-col gap-1`}>
            <div
              className={`px-4 py-2.5 rounded-2xl text-sm leading-relaxed whitespace-pre-wrap ${
                msg.role === 'user'
                  ? 'bg-primary-500 text-white rounded-br-md'
                  : 'bg-white border border-stone-200 text-stone-800 rounded-bl-md shadow-subtle'
              }`}>
              {msg.content}
            </div>
            {msg.role === 'assistant' && (msg.output_tokens > 0 || msg.cost_usd > 0) && (
              <div className="flex items-center gap-2 px-1">
                {msg.input_tokens > 0 && (
                  <span className="text-[10px] text-stone-400">
                    in: {msg.input_tokens.toLocaleString()}
                  </span>
                )}
                {msg.output_tokens > 0 && (
                  <span className="text-[10px] text-stone-400">
                    out: {msg.output_tokens.toLocaleString()}
                  </span>
                )}
                {msg.cost_usd > 0 && (
                  <span className="text-[10px] text-stone-400">{formatCost(msg.cost_usd)}</span>
                )}
              </div>
            )}
          </div>
        </div>
      ))}

      {sendingMessage && (
        <div className="flex justify-start">
          <div className="bg-white border border-stone-200 rounded-2xl rounded-bl-md px-4 py-3 shadow-subtle">
            <div className="flex items-center gap-1.5">
              <span
                className="w-1.5 h-1.5 rounded-full bg-stone-400 animate-bounce"
                style={{ animationDelay: '0ms' }}
              />
              <span
                className="w-1.5 h-1.5 rounded-full bg-stone-400 animate-bounce"
                style={{ animationDelay: '150ms' }}
              />
              <span
                className="w-1.5 h-1.5 rounded-full bg-stone-400 animate-bounce"
                style={{ animationDelay: '300ms' }}
              />
            </div>
          </div>
        </div>
      )}

      <div ref={bottomRef} />
    </div>
  );
};
