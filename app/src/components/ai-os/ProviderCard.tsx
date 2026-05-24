import { type FC } from 'react';

import { deleteProvider, selectProviderTestState, testProvider } from '../../store/aiOsSlice';
import { useAppDispatch, useAppSelector } from '../../store/hooks';
import type { UserProvider } from '../../types/aiOs';

interface ProviderCardProps {
  provider: UserProvider;
  onEdit: (provider: UserProvider) => void;
}

const KIND_LABELS: Record<string, string> = {
  openai_compatible: 'OpenAI',
  anthropic: 'Anthropic',
  google: 'Google',
};

export const ProviderCard: FC<ProviderCardProps> = ({ provider, onEdit }) => {
  const dispatch = useAppDispatch();
  const testState = useAppSelector(selectProviderTestState(provider.id));

  const handleTest = () => {
    void dispatch(testProvider(provider.id));
  };

  const handleDelete = () => {
    if (window.confirm(`Delete provider "${provider.name}"?`)) {
      void dispatch(deleteProvider(provider.id));
    }
  };

  return (
    <div className="bg-white rounded-xl border border-stone-200 p-3 shadow-sm hover:border-stone-300 transition-colors">
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className="font-medium text-sm text-stone-900 truncate">{provider.name}</span>
            <span className="flex-shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-primary-50 text-primary-700 border border-primary-100">
              {KIND_LABELS[provider.kind] ?? provider.kind}
            </span>
          </div>
          <div className="text-xs text-stone-400 truncate">{provider.default_model}</div>
        </div>
        <div className="flex-shrink-0 flex items-center gap-1">
          <div
            className={`w-2 h-2 rounded-full flex-shrink-0 ${
              provider.enabled ? 'bg-sage-400' : 'bg-stone-300'
            }`}
            title={provider.enabled ? 'Enabled' : 'Disabled'}
          />
        </div>
      </div>

      {testState.status !== 'idle' && (
        <div
          className={`mt-2 text-[11px] rounded-md px-2 py-1 ${
            testState.status === 'pending'
              ? 'bg-amber-50 text-amber-700'
              : testState.status === 'ok'
                ? 'bg-sage-50 text-sage-700'
                : 'bg-coral-50 text-coral-700'
          }`}>
          {testState.status === 'pending' && 'Testing…'}
          {testState.status === 'ok' &&
            `OK${testState.latency_ms !== undefined ? ` · ${testState.latency_ms}ms` : ''}`}
          {testState.status === 'error' && (testState.error ?? 'Test failed')}
        </div>
      )}

      <div className="mt-2 flex items-center gap-1">
        <button
          onClick={handleTest}
          disabled={testState.status === 'pending'}
          className="text-[11px] px-2 py-1 rounded-md bg-stone-100 hover:bg-stone-200 text-stone-600 transition-colors disabled:opacity-50">
          Test
        </button>
        <button
          onClick={() => onEdit(provider)}
          className="text-[11px] px-2 py-1 rounded-md bg-stone-100 hover:bg-stone-200 text-stone-600 transition-colors">
          Edit
        </button>
        <button
          onClick={handleDelete}
          className="text-[11px] px-2 py-1 rounded-md bg-stone-100 hover:bg-coral-100 text-stone-600 hover:text-coral-700 transition-colors ml-auto">
          Delete
        </button>
      </div>
    </div>
  );
};
