/**
 * Smart suggestion banner — "Jako včera?".
 *
 * Backend `get_suggestions` posílá max 3 návrhy úkolů, na kterých uživatel
 * v podobný čas trackoval v posledních 14 dnech. Banner se zobrazí jen
 * pokud existuje aspoň jeden a uživatel ho v této session ještě nedismissl
 * pro daný úkol (klíčem v sessionStorage).
 */
import { useQuery } from "@tanstack/react-query";
import { Play, Sparkles, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useOutletContext } from "react-router-dom";

import {
  getSmartSuggestionsEnabled,
  getSuggestions,
  type Suggestion,
} from "../../api/commands";
import type { ShellOutletContext } from "../Layout/AppShell";
import { formatIsoDate } from "../../lib/dates";
import { useTimerStore } from "../../stores/timerStore";

const DISMISS_KEY_PREFIX = "tracker.suggestion.dismissed:";

export function SuggestionBanner() {
  // `null` outside a router outlet (unit tests / web preview).
  const ctx = useOutletContext<ShellOutletContext | null>();
  const active = useTimerStore((s) => s.active);
  // Backend pref: when the user toggles smart suggestions off in Settings,
  // the banner stays out of the DOM entirely (no fetch, no skeleton).
  const prefQ = useQuery({
    queryKey: ["prefs", "smart-suggestions-enabled"],
    queryFn: getSmartSuggestionsEnabled,
    staleTime: Infinity,
  });
  const featureEnabled = prefQ.data ?? true;
  const q = useQuery({
    queryKey: ["smart-suggestions"],
    queryFn: getSuggestions,
    staleTime: 5 * 60_000,
    refetchInterval: 5 * 60_000,
    enabled: !active && featureEnabled,
  });
  const [dismissedKeys, setDismissedKeys] = useState<Set<string>>(() => {
    const out = new Set<string>();
    if (typeof window !== "undefined") {
      // Use the local calendar day, not the UTC slice — otherwise the
      // dismiss key shifts ±tz-offset hours and a user east of UTC sees
      // it linger past their midnight while a user west sees it expire
      // before their day rolls over.
      const day = formatIsoDate(new Date());
      for (let i = 0; i < window.sessionStorage.length; i++) {
        const k = window.sessionStorage.key(i);
        if (k?.startsWith(`${DISMISS_KEY_PREFIX}${day}:`)) {
          const issueKey = k.slice(DISMISS_KEY_PREFIX.length + day.length + 1);
          out.add(issueKey);
        }
      }
    }
    return out;
  });

  useEffect(() => {
    // Při startu timer banner schovat; query je `enabled: !active`, ale UI
    // by mohl být ještě otevřený z předchozí renderace.
  }, [active]);

  if (active) return null;
  if (!featureEnabled) return null;
  const visible = (q.data ?? []).filter((s) => !dismissedKeys.has(s.issue_key));
  if (visible.length === 0) return null;
  const top = visible[0];

  const handleAccept = async () => {
    // Go through the store (not the raw command) so `active` updates even if the
    // `timer-started` event is missed, and surface failures instead of
    // swallowing them.
    try {
      await useTimerStore.getState().start(top.issue_key);
    } catch (e) {
      ctx?.pushToast?.(
        "error",
        typeof e === "string" ? e : "Nepodařilo se spustit časomíru.",
      );
    }
  };
  const handleDismiss = () => {
    // Local-day key, matching the read path above.
    const day = formatIsoDate(new Date());
    window.sessionStorage.setItem(
      `${DISMISS_KEY_PREFIX}${day}:${top.issue_key}`,
      "1",
    );
    setDismissedKeys((prev) => new Set([...prev, top.issue_key]));
  };

  return (
    <div
      role="status"
      aria-live="polite"
      className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-md)]"
      style={{
        background: "var(--accent-soft)",
        border: "1px solid color-mix(in srgb, var(--accent) 35%, transparent)",
      }}
    >
      <Sparkles
        className="w-4 h-4 text-[var(--accent)] shrink-0"
        aria-hidden
      />
      <div className="flex-1 min-w-0 text-xs">
        <span className="text-[var(--text-primary)]">
          Začít{" "}
          <span className="font-mono font-semibold">{top.issue_key}</span>
          {top.summary ? ` — ${top.summary}` : ""}?
        </span>
        <span className="text-[var(--text-tertiary)] ml-1">
          (před tím v {labelHour(top.bucket_hour)} {top.occurrences}× v
          posledních 14 dnech)
        </span>
      </div>
      <button
        type="button"
        onClick={() => void handleAccept()}
        className="inline-flex items-center gap-1 h-7 px-2.5 rounded-[var(--radius-sm)]
                   text-[11px] font-medium transition-colors duration-150"
        style={{
          background: "var(--accent)",
          color: "var(--accent-text, #fff)",
        }}
      >
        <Play className="w-3 h-3" aria-hidden />
        Spustit
      </button>
      <button
        type="button"
        onClick={handleDismiss}
        title="Skrýt pro dnešek"
        aria-label="Skrýt"
        className="h-7 w-7 rounded-[var(--radius-sm)] flex items-center justify-center
                   text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]
                   transition-colors duration-150"
      >
        <X className="w-3.5 h-3.5" aria-hidden />
      </button>
    </div>
  );
}

function labelHour(h: number): string {
  return `${h.toString().padStart(2, "0")}:00`;
}

// Silence unused-import warning if Suggestion isn't picked up via re-export.
export type { Suggestion };
