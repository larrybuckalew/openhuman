import { type FC, useState } from 'react';

import { fetchUsage } from '../../store/aiOsSlice';
import { useAppDispatch } from '../../store/hooks';
import type { UsageSummary } from '../../types/aiOs';

interface TokenUsagePanelProps {
  usage: UsageSummary | null;
}

const PERIOD_OPTIONS: { label: string; days: number }[] = [
  { label: '7d', days: 7 },
  { label: '30d', days: 30 },
  { label: '90d', days: 90 },
];

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`;
  return String(n);
}

function formatCostDisplay(usd: number): string {
  return `$${usd.toFixed(4)}`;
}

export const TokenUsagePanel: FC<TokenUsagePanelProps> = ({ usage }) => {
  const dispatch = useAppDispatch();
  const [activeDays, setActiveDays] = useState(30);

  const handlePeriodChange = (days: number) => {
    setActiveDays(days);
    void dispatch(fetchUsage(days));
  };

  const maxCost =
    usage && usage.by_provider.length > 0
      ? Math.max(...usage.by_provider.map(p => p.total_cost_usd))
      : 0;

  return (
    <div className="flex-1 overflow-y-auto px-6 py-6">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-base font-semibold text-stone-900">Token Usage</h2>
        <div className="flex items-center gap-1 bg-stone-100 rounded-lg p-0.5">
          {PERIOD_OPTIONS.map(opt => (
            <button
              key={opt.days}
              onClick={() => handlePeriodChange(opt.days)}
              className={`text-xs px-3 py-1.5 rounded-md font-medium transition-colors ${
                activeDays === opt.days
                  ? 'bg-white text-stone-800 shadow-subtle'
                  : 'text-stone-500 hover:text-stone-700'
              }`}>
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      {!usage ? (
        <div className="text-sm text-stone-400 text-center py-12">Loading usage data…</div>
      ) : (
        <div className="space-y-6">
          <div className="bg-stone-50 rounded-2xl border border-stone-200 p-5">
            <div className="text-xs text-stone-400 font-medium mb-1">
              Total spend ({usage.period_days}d)
            </div>
            <div className="text-3xl font-semibold text-stone-900 font-mono">
              {formatCostDisplay(usage.total_cost_usd)}
            </div>
            <div className="mt-2 flex items-center gap-4 text-xs text-stone-500">
              <span>
                <span className="font-medium">{formatTokens(usage.total_input_tokens)}</span> in
              </span>
              <span>
                <span className="font-medium">{formatTokens(usage.total_output_tokens)}</span> out
              </span>
            </div>
          </div>

          {usage.by_provider.length > 0 && (
            <div>
              <div className="text-xs font-semibold text-stone-400 uppercase tracking-widest mb-3">
                By Provider
              </div>
              <div className="space-y-3">
                {usage.by_provider.map(p => {
                  const barPct = maxCost > 0 ? (p.total_cost_usd / maxCost) * 100 : 0;
                  return (
                    <div key={p.provider_id}>
                      <div className="flex items-center justify-between mb-1">
                        <span className="text-sm text-stone-700 font-medium truncate max-w-[60%]">
                          {p.provider_name}
                        </span>
                        <span className="text-xs font-mono text-stone-500">
                          {formatCostDisplay(p.total_cost_usd)}
                        </span>
                      </div>
                      <div className="h-2 bg-stone-200 rounded-full overflow-hidden">
                        <div
                          className="h-full bg-primary-400 rounded-full transition-all duration-500"
                          style={{ width: `${barPct}%` }}
                        />
                      </div>
                      <div className="flex items-center gap-3 mt-1 text-[10px] text-stone-400">
                        <span>{formatTokens(p.total_input_tokens)} in</span>
                        <span>{formatTokens(p.total_output_tokens)} out</span>
                        <span>{p.request_count} reqs</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {usage.by_provider.length === 0 && (
            <div className="text-sm text-stone-400 text-center py-8">
              No usage data for this period.
            </div>
          )}
        </div>
      )}
    </div>
  );
};
