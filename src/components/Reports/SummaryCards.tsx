/**
 * The four-card summary row at the top of the Reports view.
 *
 * Phase 18B — Item 22: the earnings card's hide/show toggle is now persisted
 * via the `earnings_visible` user pref so it survives reloads.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Eye, EyeOff } from "lucide-react";

import { getEarningsVisible, setEarningsVisible } from "../../api/commands";
import { formatMoney } from "../../lib/format";

export interface SummaryCardsProps {
  totalSeconds: number;
  daysWorked: number;
  issuesTouched: number;
  earnings: number;
  currency: string;
  hourlyRateConfigured: boolean;
  durationLabel: React.ReactNode;
}

export function SummaryCards(props: SummaryCardsProps) {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
      <BigStatCard label="Celkový čas" value={props.durationLabel} />
      <BigStatCard label="Odpracovaných dní" value={`${props.daysWorked}`} />
      <BigStatCard label="Dotčených úkolů" value={`${props.issuesTouched}`} />
      <EarningsCard
        earnings={props.earnings}
        currency={props.currency}
        enabled={props.hourlyRateConfigured}
      />
    </div>
  );
}

function BigStatCard({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-4">
      <div className="text-[11px] text-[var(--text-tertiary)]">{label}</div>
      <div className="mt-2 text-2xl font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function EarningsCard({
  earnings,
  currency,
  enabled,
}: {
  earnings: number;
  currency: string;
  enabled: boolean;
}) {
  const queryClient = useQueryClient();
  const visQ = useQuery({
    queryKey: ["earnings-visible"],
    queryFn: getEarningsVisible,
    staleTime: 30_000,
  });
  const revealed = visQ.data ?? true;

  const toggle = async () => {
    const next = !revealed;
    try {
      await setEarningsVisible(next);
    } finally {
      queryClient.invalidateQueries({ queryKey: ["earnings-visible"] });
    }
  };

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-4 relative">
      <div className="flex items-center justify-between">
        <div className="text-[11px] text-[var(--text-tertiary)]">Výdělek</div>
        {enabled && (
          <button
            type="button"
            onClick={toggle}
            aria-label={revealed ? "Skrýt výdělek" : "Zobrazit výdělek"}
            className="text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]
                       transition-colors duration-150"
          >
            {revealed ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
          </button>
        )}
      </div>
      <div className="mt-2 text-2xl font-semibold tabular-nums text-[var(--accent-2)]">
        {!enabled ? (
          <span className="text-[var(--text-tertiary)]">—</span>
        ) : revealed ? (
          formatMoney(earnings, currency)
        ) : (
          <span aria-hidden className="tracking-wider">
            ••••
          </span>
        )}
      </div>
    </div>
  );
}
