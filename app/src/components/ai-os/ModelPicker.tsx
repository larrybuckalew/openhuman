import { type FC, useEffect } from 'react';

import {
  fetchModels,
  selectModelsForProvider,
  selectModelsStatus,
} from '../../store/aiOsSlice';
import { useAppDispatch, useAppSelector } from '../../store/hooks';

interface ModelPickerProps {
  providerId: string;
  value: string;
  onChange: (modelId: string) => void;
  className?: string;
}

export const ModelPicker: FC<ModelPickerProps> = ({ providerId, value, onChange, className }) => {
  const dispatch = useAppDispatch();
  const models = useAppSelector(selectModelsForProvider(providerId));
  const status = useAppSelector(selectModelsStatus(providerId));

  useEffect(() => {
    if (providerId && status === 'idle') {
      void dispatch(fetchModels(providerId));
    }
  }, [dispatch, providerId, status]);

  const baseSelectClass =
    'text-[11px] font-mono rounded-md border border-stone-200 bg-white text-stone-700 px-2 py-1 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent truncate';

  const baseFallbackClass =
    'text-[11px] font-mono rounded-md border border-stone-200 bg-white text-stone-700 px-2 py-1 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent w-40';

  if (status === 'loading') {
    return (
      <select disabled className={`${baseSelectClass} opacity-60 ${className ?? ''}`}>
        <option>Loading models…</option>
      </select>
    );
  }

  if (status === 'succeeded' && models.length > 0) {
    return (
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        className={`${baseSelectClass} ${className ?? ''}`}
        title="Select model">
        {models.map(m => (
          <option key={m.id} value={m.id}>
            {m.name}
            {m.context_length != null
              ? `  ${m.context_length >= 1000 ? `${Math.round(m.context_length / 1000)}k` : String(m.context_length)}`
              : ''}
          </option>
        ))}
      </select>
    );
  }

  // Fallback: empty models list (provider offline) or failed status — free-text input
  return (
    <input
      type="text"
      value={value}
      onChange={e => onChange(e.target.value)}
      placeholder="Model ID"
      className={`${baseFallbackClass} ${className ?? ''}`}
      title="Enter model ID manually"
    />
  );
};
