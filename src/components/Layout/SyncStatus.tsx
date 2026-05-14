/**
 * Header pill that surfaces the cache-sync state.
 *
 * Three states:
 * - idle: shows "Synced <relative-time>" or "Never synced" if no timestamp.
 * - syncing: spinner + "Syncing…".
 * - error: red dot + clickable for the user to inspect details (we just
 *   show the message inline; the parent can implement a fancier UI later).
 */
import { clsx } from "clsx";
import { AlertCircle, RefreshCw } from "lucide-react";

import { formatRelativeTime } from "../../lib/format";
import { useNow } from "../../hooks/useNow";
import { Spinner } from "../common/Spinner";

export type SyncState =
  | { kind: "idle"; lastSyncMs: number | null }
  | { kind: "syncing" }
  | { kind: "error"; message: string };

export interface SyncStatusProps {
  state: SyncState;
  /** Triggered when the user clicks the pill. */
  onRefresh: () => void;
  className?: string;
}

export function SyncStatus({ state, onRefresh, className }: SyncStatusProps) {
  // Re-render once a minute so the relative timestamp updates.
  const nowMs = useNow(60_000);

  let content: React.ReactNode;
  if (state.kind === "syncing") {
    content = (
      <>
        <Spinner className="w-3 h-3" />
        <span className="text-xs">Syncing…</span>
      </>
    );
  } else if (state.kind === "error") {
    content = (
      <>
        <AlertCircle className="w-3.5 h-3.5 text-red-400" aria-hidden />
        <span className="text-xs text-red-300" title={state.message}>
          Sync failed
        </span>
      </>
    );
  } else {
    content = (
      <>
        <RefreshCw className="w-3.5 h-3.5 text-neutral-400" aria-hidden />
        <span className="text-xs text-neutral-300">
          {state.lastSyncMs
            ? `Synced ${formatRelativeTime(state.lastSyncMs, new Date(nowMs))}`
            : "Never synced"}
        </span>
      </>
    );
  }

  return (
    <button
      type="button"
      onClick={onRefresh}
      disabled={state.kind === "syncing"}
      aria-label="Refresh issue cache"
      className={clsx(
        "inline-flex items-center gap-1.5 px-2 py-1 rounded-full border transition-colors",
        state.kind === "error"
          ? "border-red-700/40 bg-red-900/10 hover:bg-red-900/20"
          : "border-neutral-800 hover:bg-neutral-800",
        state.kind === "syncing" ? "cursor-default" : "cursor-pointer",
        className,
      )}
    >
      {content}
    </button>
  );
}
