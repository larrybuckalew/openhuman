import { type FC, useState } from 'react';

import { addProvider, updateProvider } from '../../store/aiOsSlice';
import { useAppDispatch } from '../../store/hooks';
import type { ProviderKind, UserProvider } from '../../types/aiOs';
import { PROVIDER_TEMPLATES } from './ProviderTemplates';

interface AddProviderModalProps {
  onClose: () => void;
  editingProvider?: UserProvider | null;
}

const HERMES_OPENCLAW = ['Hermès', 'Openclaw'];

export const AddProviderModal: FC<AddProviderModalProps> = ({ onClose, editingProvider }) => {
  const dispatch = useAppDispatch();

  const [name, setName] = useState(editingProvider?.name ?? '');
  const [kind, setKind] = useState<ProviderKind>(editingProvider?.kind ?? 'openai_compatible');
  const [baseUrl, setBaseUrl] = useState(editingProvider?.base_url ?? '');
  const [vpsUrl, setVpsUrl] = useState(editingProvider?.vps_url ?? '');
  const [apiKey, setApiKey] = useState(editingProvider?.api_key ?? '');
  const [defaultModel, setDefaultModel] = useState(editingProvider?.default_model ?? '');
  const [showApiKey, setShowApiKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isDualEndpoint = HERMES_OPENCLAW.some(n => name.toLowerCase().includes(n.toLowerCase()));

  const showVpsUrlField =
    kind === 'openai_compatible' &&
    (isDualEndpoint ||
      !baseUrl.trim() ||
      !['openai.com', 'anthropic.com', 'googleapis.com'].some(domain =>
        baseUrl.includes(domain)
      ));

  const handleTemplateClick = (template: (typeof PROVIDER_TEMPLATES)[number]) => {
    setName(template.name);
    setKind(template.kind);
    setBaseUrl(template.base_url);
    setDefaultModel(template.default_model);
    if (!template.requires_api_key) {
      setApiKey('');
    }
  };

  const handleSave = async () => {
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      if (editingProvider) {
        await dispatch(
          updateProvider({
            id: editingProvider.id,
            name: name.trim(),
            kind,
            base_url: baseUrl.trim(),
            api_key: apiKey.trim() || undefined,
            default_model: defaultModel.trim(),
            vps_url: vpsUrl.trim() || undefined,
          })
        ).unwrap();
      } else {
        await dispatch(
          addProvider({
            name: name.trim(),
            kind,
            base_url: baseUrl.trim(),
            api_key: apiKey.trim() || undefined,
            default_model: defaultModel.trim(),
            enabled: true,
            vps_url: vpsUrl.trim() || undefined,
          })
        ).unwrap();
      }
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save provider');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/40 backdrop-blur-sm">
      <div className="bg-white rounded-2xl shadow-large w-full max-w-lg mx-4 overflow-hidden">
        <div className="px-5 pt-5 pb-4 border-b border-stone-100">
          <h2 className="text-base font-semibold text-stone-900">
            {editingProvider ? 'Edit Provider' : 'Add Provider'}
          </h2>
        </div>

        {!editingProvider && (
          <div className="px-5 pt-4">
            <p className="text-xs text-stone-400 mb-2 font-medium uppercase tracking-wider">
              Quick setup
            </p>
            <div className="grid grid-cols-4 gap-2">
              {PROVIDER_TEMPLATES.map(t => (
                <button
                  key={t.name}
                  onClick={() => handleTemplateClick(t)}
                  className="flex flex-col items-center gap-1 p-2 rounded-xl border border-stone-200 hover:border-primary-300 hover:bg-primary-50 transition-colors text-center group">
                  <span className="text-[11px] font-medium text-stone-700 group-hover:text-primary-700 leading-tight">
                    {t.name}
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}

        <div className="px-5 py-4 space-y-3">
          <div>
            <label className="block text-xs font-medium text-stone-600 mb-1">Name</label>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="My Provider"
              className="w-full text-sm px-3 py-2 rounded-lg border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-stone-600 mb-1">Kind</label>
            <select
              value={kind}
              onChange={e => setKind(e.target.value as ProviderKind)}
              className="w-full text-sm px-3 py-2 rounded-lg border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent bg-white">
              <option value="openai_compatible">OpenAI Compatible</option>
              <option value="anthropic">Anthropic</option>
              <option value="google">Google</option>
            </select>
          </div>

          <div>
            <label className="block text-xs font-medium text-stone-600 mb-1">
              {isDualEndpoint ? 'Local URL' : 'Base URL'}
            </label>
            <input
              type="text"
              value={baseUrl}
              onChange={e => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com/v1"
              className="w-full text-sm px-3 py-2 rounded-lg border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent font-mono"
            />
          </div>

          {showVpsUrlField && (
            <div>
              <label className="block text-xs font-medium text-stone-600 mb-1">
                VPS URL
                <span className="ml-1 text-stone-400 font-normal">(optional)</span>
              </label>
              <input
                type="text"
                value={vpsUrl}
                onChange={e => setVpsUrl(e.target.value)}
                placeholder="http://your-vps:port/v1"
                className="w-full text-sm px-3 py-2 rounded-lg border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent font-mono"
              />
            </div>
          )}

          <div>
            <label className="block text-xs font-medium text-stone-600 mb-1">
              API Key
              <span className="ml-1 text-stone-400 font-normal">(optional)</span>
            </label>
            <div className="relative">
              <input
                type={showApiKey ? 'text' : 'password'}
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                placeholder="sk-..."
                className="w-full text-sm px-3 py-2 pr-10 rounded-lg border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent font-mono"
              />
              <button
                type="button"
                onClick={() => setShowApiKey(v => !v)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-stone-400 hover:text-stone-600 transition-colors">
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  {showApiKey ? (
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.8}
                      d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
                    />
                  ) : (
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.8}
                      d="M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                    />
                  )}
                </svg>
              </button>
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-stone-600 mb-1">Default Model</label>
            <input
              type="text"
              value={defaultModel}
              onChange={e => setDefaultModel(e.target.value)}
              placeholder="gpt-4o"
              className="w-full text-sm px-3 py-2 rounded-lg border border-stone-200 focus:outline-none focus:ring-2 focus:ring-primary-300 focus:border-transparent font-mono"
            />
          </div>

          {error && (
            <div className="text-xs text-coral-600 bg-coral-50 rounded-lg px-3 py-2">{error}</div>
          )}
        </div>

        <div className="px-5 pb-5 flex items-center justify-end gap-2">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-stone-600 hover:text-stone-800 transition-colors">
            Cancel
          </button>
          <button
            onClick={() => void handleSave()}
            disabled={saving}
            className="px-4 py-2 text-sm font-medium bg-primary-500 hover:bg-primary-600 text-white rounded-lg transition-colors disabled:opacity-50">
            {saving ? 'Saving…' : editingProvider ? 'Save Changes' : 'Add Provider'}
          </button>
        </div>
      </div>
    </div>
  );
};
