/**
 * Top-level shell for the main app routes (Today, History, Reports, Settings).
 *
 * Responsibilities:
 *   - Renders the TopBar (timer chip + search + sync + settings shortcut).
 *   - Renders the SideNav (icon rail).
 *   - Hosts the global StopDialog so the timer can be stopped from any route.
 *   - Hosts a toast region for cross-route notifications.
 *   - Owns the sync state (refresh_all spinner) and broadcasts toasts on
 *     worklog-saved / worklog-error.
 *   - Owns the CommandPalette and Cmd/Ctrl+K shortcut.
 *
 * The actual route content is rendered via React Router's `<Outlet />`.
 */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { Outlet, useNavigate } from "react-router-dom";

import { hasConfig, refreshAll, startTimer } from "../../api/commands";
import type { ActiveTimerState, WorklogRow } from "../../api/types";
import { useTauriEvent } from "../../hooks/useTauriEvent";
import { usePrefsStore } from "../../stores/prefsStore";
import { useTimerStore } from "../../stores/timerStore";
import {
  Toaster,
  type Toast,
  type ToastVariant,
} from "../common/Toast";
import { StopDialog } from "../Timer/TimerControls";
import { CommandPalette } from "./CommandPalette";
import { SideNav } from "./SideNav";
import type { SyncState } from "./SyncStatus";
import { TopBar } from "./TopBar";

/** Number of days of worklog history we pull on startup / manual refresh. */
const REFRESH_WINDOW_DAYS = 30;

export function AppShell() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const hydrateTimer = useTimerStore((s) => s.hydrate);
  const hydratePrefs = usePrefsStore((s) => s.hydrate);
  const active = useTimerStore((s) => s.active);
  const timerBusy = useTimerStore((s) => s.busy);
  const setActive = useTimerStore((s) => s.setActive);

  // Hydrate stores once.
  useEffect(() => {
    void hydrateTimer();
    void hydratePrefs();
  }, [hydrateTimer, hydratePrefs]);

  // ---- sync state ----------------------------------------------------------
  const [syncState, setSyncState] = useState<SyncState>({
    kind: "idle",
    lastSyncMs: null,
  });
  const refresh = useCallback(async () => {
    setSyncState({ kind: "syncing" });
    try {
      await refreshAll(REFRESH_WINDOW_DAYS);
      setSyncState({ kind: "idle", lastSyncMs: Date.now() });
      // Bust any TanStack Query cache keys that depend on backend data.
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
      queryClient.invalidateQueries({ queryKey: ["search-issues"] });
      queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
      queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
    } catch (e) {
      const message =
        typeof e === "string" ? e : (e as Error).message ?? "Refresh failed";
      setSyncState({ kind: "error", message });
    }
  }, [queryClient]);

  // ---- toast notifications -------------------------------------------------
  const toastIdRef = useRef(1);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const pushToast = useCallback((variant: ToastVariant, message: string) => {
    const id = toastIdRef.current++;
    setToasts((prev) => [...prev, { id, variant, message }]);
  }, []);
  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  // ---- backend events ------------------------------------------------------
  const onWorklogSaved = useCallback(
    (row: WorklogRow) => {
      const minutes = Math.max(1, Math.round(row.duration_s / 60));
      const dur =
        minutes < 60
          ? `${minutes}m`
          : minutes % 60 === 0
            ? `${minutes / 60}h`
            : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
      pushToast("success", `Saved ${dur} on ${row.issue_key}.`);
      queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
      queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
      setStopOpen(false);
    },
    [pushToast, queryClient],
  );
  useTauriEvent<WorklogRow>("worklog-saved", onWorklogSaved);

  const onWorklogError = useCallback(
    (err: unknown) => {
      const msg = typeof err === "string" ? err : "Jira sync failed";
      pushToast("error", `Worklog: ${msg}`);
    },
    [pushToast],
  );
  useTauriEvent<unknown>("worklog-error", onWorklogError);

  const onCacheRefreshed = useCallback(() => {
    setSyncState({ kind: "idle", lastSyncMs: Date.now() });
    queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
    queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
  }, [queryClient]);
  useTauriEvent<number>("cache-refreshed", onCacheRefreshed);

  // Auto-sync event fired by backend ~3s after startup.
  const onAutoSyncComplete = useCallback(() => {
    setSyncState({ kind: "idle", lastSyncMs: Date.now() });
    queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
    queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
    queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
    queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
  }, [queryClient]);
  useTauriEvent<unknown>("auto-sync-complete", onAutoSyncComplete);

  const onPrefsChanged = useCallback(() => {
    void hydratePrefs();
  }, [hydratePrefs]);
  useTauriEvent<string>("prefs-changed", onPrefsChanged);

  const onTimerStarted = useCallback(
    (snap: ActiveTimerState) => {
      setActive(snap);
    },
    [setActive],
  );
  useTauriEvent<ActiveTimerState>("timer-started", onTimerStarted);

  // When config changes (e.g. sign-out from Settings) bounce to setup.
  const onConfigChanged = useCallback(() => {
    hasConfig()
      .then((ok) => {
        if (!ok) navigate("/setup", { replace: true });
      })
      .catch(() => {
        /* ignore — best-effort */
      });
  }, [navigate]);
  useTauriEvent<unknown>("config-changed", onConfigChanged);

  // ---- StopDialog state ----------------------------------------------------
  const [stopOpen, setStopOpen] = useState(false);

  const handleStopConfirm = useCallback(
    async ({
      comment,
      startedAtMs,
    }: {
      comment: string;
      startedAtMs: number | null;
    }) => {
      try {
        if (startedAtMs !== null) {
          await useTimerStore.getState().updateStart(startedAtMs);
        }
        await useTimerStore
          .getState()
          .stop(comment.length > 0 ? comment : undefined);
        // Toast fires via the worklog-saved event handler above.
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Failed to save worklog");
      }
    },
    [pushToast],
  );

  // ---- CommandPalette state + Cmd/Ctrl+K shortcut --------------------------
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const isMac =
        typeof navigator !== "undefined" &&
        /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");
      const mod = isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
      if (mod && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      } else if (e.key === "Escape" && paletteOpen) {
        setPaletteOpen(false);
      }
      // Refresh stays on Cmd/Ctrl+R.
      if (mod && (e.key === "r" || e.key === "R")) {
        e.preventDefault();
        void refresh();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [refresh, paletteOpen]);

  const handlePaletteStart = useCallback(
    async (issueKey: string) => {
      try {
        await startTimer(issueKey);
        navigate("/");
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Failed to start timer");
      }
    },
    [navigate, pushToast],
  );

  return (
    <div className="h-screen flex flex-col bg-[#0f0f0f] text-neutral-100">
      <TopBar
        syncState={syncState}
        onRefresh={refresh}
        onOpenCommandPalette={() => setPaletteOpen(true)}
        onStop={active ? () => setStopOpen(true) : undefined}
      />
      <div className="flex-1 flex min-h-0">
        <SideNav />
        <main className="flex-1 min-w-0 overflow-y-auto">
          <Outlet context={{ pushToast, openStopDialog: () => setStopOpen(true) }} />
        </main>
      </div>

      {active && (
        <StopDialog
          open={stopOpen}
          active={active}
          busy={timerBusy}
          onClose={() => setStopOpen(false)}
          onConfirm={handleStopConfirm}
        />
      )}

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onStartIssue={handlePaletteStart}
      />

      <Toaster toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}

/** Shape of the data passed to nested routes via `useOutletContext`. */
export interface ShellOutletContext {
  pushToast: (variant: ToastVariant, message: string) => void;
  openStopDialog: () => void;
}
