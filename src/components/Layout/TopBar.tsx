/**
 * Top application bar.
 *
 * - left: app name + live timer chip (clickable to "/" → Today).
 * - center: global "search / jump" trigger that opens the CommandPalette.
 * - right: sync status, settings shortcut.
 *
 * The timer chip is live-ticking so it works from any route.
 */
import { Search, Settings as SettingsIcon, Square } from "lucide-react";
import { useNavigate } from "react-router-dom";

import { useNow } from "../../hooks/useNow";
import { formatDuration } from "../../lib/format";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";
import { IconButton } from "../common/IconButton";
import { SyncStatus, type SyncState } from "./SyncStatus";

export interface TopBarProps {
  syncState: SyncState;
  onRefresh: () => void;
  /** Open the command palette / jump-to search. */
  onOpenCommandPalette: () => void;
  /** Triggered when the user clicks the running timer's stop icon. */
  onStop?: () => void;
}

export function TopBar({
  syncState,
  onRefresh,
  onOpenCommandPalette,
  onStop,
}: TopBarProps) {
  const navigate = useNavigate();
  const active = useTimerStore((s) => s.active);
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = elapsedSeconds(active, now);
  const isMac =
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");
  const mod = isMac ? "⌘" : "Ctrl";

  return (
    <header className="flex items-center justify-between gap-3 px-4 h-12 border-b border-neutral-800/70 bg-neutral-950/60 backdrop-blur shrink-0">
      <div className="flex items-center gap-3 min-w-0">
        <button
          type="button"
          onClick={() => navigate("/")}
          className="text-sm font-semibold tracking-tight hover:text-sky-300 transition-colors"
          aria-label="Tracker home"
        >
          Tracker
        </button>

        {active ? (
          <button
            type="button"
            onClick={() => navigate("/")}
            aria-label={`Tracking ${active.issue_key}, elapsed ${formatDuration(elapsed)}`}
            className="inline-flex items-center gap-2 pl-2 pr-1 py-1 rounded-full bg-emerald-600/10 border border-emerald-600/30 hover:bg-emerald-600/20 transition-colors"
          >
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
                onClick={(e) => {
                  e.stopPropagation();
                  onStop();
                }}
                aria-label="Stop timer"
                className="ml-0.5 w-5 h-5 inline-flex items-center justify-center rounded-full text-emerald-200 hover:text-white hover:bg-emerald-700/40"
              >
                <Square className="w-3 h-3" aria-hidden />
              </button>
            )}
          </button>
        ) : (
          <span className="inline-flex items-center gap-2 pl-2 pr-2.5 py-1 rounded-full border border-neutral-800/80 text-[11px] text-neutral-500">
            <span aria-hidden className="w-1.5 h-1.5 rounded-full bg-neutral-700" />
            Idle
          </span>
        )}
      </div>

      <button
        type="button"
        onClick={onOpenCommandPalette}
        className="hidden md:inline-flex items-center gap-2 px-2.5 py-1 rounded-md border border-neutral-800 bg-neutral-950/80 text-xs text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200 transition-colors min-w-[260px]"
        aria-label="Search or jump"
      >
        <Search className="w-3.5 h-3.5" aria-hidden />
        <span className="flex-1 text-left">Search issues, jump to view…</span>
        <kbd className="text-[10px] font-mono text-neutral-500 border border-neutral-700 rounded px-1">
          {mod}K
        </kbd>
      </button>

      <div className="flex items-center gap-2">
        <SyncStatus state={syncState} onRefresh={onRefresh} />
        <IconButton
          aria-label="Settings"
          onClick={() => navigate("/settings")}
        >
          <SettingsIcon className="w-4 h-4" aria-hidden />
        </IconButton>
      </div>
    </header>
  );
}
