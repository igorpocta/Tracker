/**
 * App header bar.
 *
 * Three regions:
 * - left: app brand + current timer chip (live-ticking even on sub-views).
 * - center: optional content (currently unused).
 * - right: sync status pill, settings link.
 */
import { Settings, Square } from "lucide-react";

import { useNow } from "../../hooks/useNow";
import { formatDuration } from "../../lib/format";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";
import { IconButton } from "../common/IconButton";
import { SyncStatus, type SyncState } from "./SyncStatus";

export interface HeaderProps {
  syncState: SyncState;
  onRefresh: () => void;
  /** Triggered when the user clicks the gear icon. */
  onOpenSettings: () => void;
  /** Triggered when the user clicks the running timer's stop icon. */
  onStop?: () => void;
}

export function Header({
  syncState,
  onRefresh,
  onOpenSettings,
  onStop,
}: HeaderProps) {
  const active = useTimerStore((s) => s.active);
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = elapsedSeconds(active, now);

  return (
    <header className="flex items-center justify-between px-4 h-12 border-b border-neutral-800/70 bg-neutral-950/60 backdrop-blur shrink-0">
      <div className="flex items-center gap-3 min-w-0">
        <h1 className="text-sm font-semibold tracking-tight">Tracker</h1>

        {active && (
          <div className="inline-flex items-center gap-2 pl-2 pr-1 py-1 rounded-full bg-emerald-600/10 border border-emerald-600/30">
            <span
              aria-hidden
              className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"
            />
            <span className="font-mono text-[11px] text-neutral-200">
              {active.issue_key}
            </span>
            <span className="font-mono tabular-nums text-xs text-emerald-300">
              {formatDuration(elapsed)}
            </span>
            {onStop && (
              <button
                type="button"
                onClick={onStop}
                aria-label="Stop timer"
                className="ml-0.5 w-5 h-5 inline-flex items-center justify-center rounded-full text-emerald-200 hover:text-white hover:bg-emerald-700/40"
              >
                <Square className="w-3 h-3" aria-hidden />
              </button>
            )}
          </div>
        )}
      </div>

      <div className="flex items-center gap-2">
        <SyncStatus state={syncState} onRefresh={onRefresh} />
        <IconButton aria-label="Settings" onClick={onOpenSettings}>
          <Settings className="w-4 h-4" aria-hidden />
        </IconButton>
      </div>
    </header>
  );
}
