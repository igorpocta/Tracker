/**
 * Top-level shell for the main app routes.
 *
 * Phase 13 layout — matches the original Tracker reference:
 *
 *   ┌────────────────────────────────────────────────────────────────────┐
 *   │ T│  Start tracking…                     19:57  [▶ Start]   │   ⊕  │
 *   │ ⏰│  ─────────────────────────────────────────────────       │ Add │
 *   │ 📊│  Route content                                            │ entry│
 *   │ 📅│                                                            │ panel│
 *   │ 🎯│                                                            │ (opt)│
 *   │ ⚙ │                                                            │      │
 *   │ ◯ │       ⌘, Settings  ⌘R Refresh  ⌘I Re-index  ⌘N New entry   │      │
 *   └────────────────────────────────────────────────────────────────────┘
 *
 * Composed from:
 *   • IconSidebar       — thin always-dark left rail
 *   • StartTrackingBar  — top input + clock + start/stop
 *   • <Outlet>          — route content
 *   • CommandBar        — bottom keyboard hint pill
 *   • AddEntryPanel     — optional right-side slide-in
 *
 * Settings has its own internal sidebar so we render WITHOUT the top
 * StartTrackingBar and CommandBar in that case (the Settings page is a
 * focus surface — no chrome competing with it).
 */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";

import {
  createManualWorklog,
  hasConfig,
  refreshAll,
  refreshCache,
  startTimer,
} from "../../api/commands";
import type { ActiveTimerState, WorklogRow } from "../../api/types";
import { useActivityTracker } from "../../hooks/useActivityTracker";
import { useTauriEvent } from "../../hooks/useTauriEvent";
import { usePrefsStore } from "../../stores/prefsStore";
import { useTimerStore } from "../../stores/timerStore";
import { AddEntryPanel } from "../Entry/AddEntryPanel";
import {
  Toaster,
  type Toast,
  type ToastUndoAction,
  type ToastVariant,
} from "../common/Toast";
import { StopDialog } from "../Timer/TimerControls";
import { CommandBar } from "./CommandBar";
import { IconSidebar } from "./IconSidebar";
import { StartTrackingBar } from "./StartTrackingBar";

/** Number of days of worklog history we pull on startup / manual refresh. */
const REFRESH_WINDOW_DAYS = 30;

export interface ShellOutletContext {
  pushToast: (
    variant: ToastVariant,
    message: string,
    opts?: { undo?: ToastUndoAction; ttlMs?: number },
  ) => void;
  openStopDialog: () => void;
  openAddEntry: () => void;
}

export function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const queryClient = useQueryClient();

  const hydrateTimer = useTimerStore((s) => s.hydrate);
  const hydratePrefs = usePrefsStore((s) => s.hydrate);
  const active = useTimerStore((s) => s.active);
  const timerBusy = useTimerStore((s) => s.busy);
  const setActive = useTimerStore((s) => s.setActive);

  // Phase 18A — Item 32: record user activity at the shell level so every
  // route benefits without each component needing to wire its own listeners.
  useActivityTracker();

  // Hydrate stores once.
  useEffect(() => {
    void hydrateTimer();
    void hydratePrefs();
  }, [hydrateTimer, hydratePrefs]);

  // ---- refresh -------------------------------------------------------------
  const refresh = useCallback(async () => {
    try {
      await refreshAll(REFRESH_WINDOW_DAYS);
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
      queryClient.invalidateQueries({ queryKey: ["search-issues"] });
      queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
      queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
    } catch {
      /* swallow — toast lives elsewhere */
    }
  }, [queryClient]);

  const reindex = useCallback(async () => {
    try {
      await refreshCache();
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["search-issues"] });
    } catch {
      /* swallow */
    }
  }, [queryClient]);

  // ---- toast notifications -------------------------------------------------
  const toastIdRef = useRef(1);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const pushToast = useCallback(
    (
      variant: ToastVariant,
      message: string,
      opts?: { undo?: ToastUndoAction; ttlMs?: number },
    ) => {
      const id = toastIdRef.current++;
      setToasts((prev) => [
        ...prev,
        { id, variant, message, undo: opts?.undo, ttlMs: opts?.ttlMs },
      ]);
    },
    [],
  );
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
      pushToast("success", `Uloženo ${dur} na ${row.issue_key}.`);
      queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
      queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
      setStopOpen(false);
    },
    [pushToast, queryClient],
  );
  useTauriEvent<WorklogRow>("worklog-saved", onWorklogSaved);

  // Phase 15 — mutation events. All four invalidate the same query keys so the
  // visible Tímové Log refreshes immediately.
  const invalidateWorklogQueries = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
    queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
  }, [queryClient]);
  useTauriEvent<WorklogRow>("worklog-created", invalidateWorklogQueries);
  useTauriEvent<WorklogRow>("worklog-updated", invalidateWorklogQueries);
  useTauriEvent<WorklogRow>("worklog-deleted", invalidateWorklogQueries);
  useTauriEvent<WorklogRow>("worklog-undo-deleted", invalidateWorklogQueries);
  useTauriEvent<string>("worklog-delete-committed", invalidateWorklogQueries);
  useTauriEvent<WorklogRow>("worklog-moved", invalidateWorklogQueries);

  const onWorklogError = useCallback(
    (err: unknown) => {
      const msg = typeof err === "string" ? err : "Synchronizace s Jirou selhala";
      pushToast("error", `Záznam: ${msg}`);
    },
    [pushToast],
  );
  useTauriEvent<unknown>("worklog-error", onWorklogError);

  const onCacheRefreshed = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
    queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
  }, [queryClient]);
  useTauriEvent<number>("cache-refreshed", onCacheRefreshed);

  // Auto-sync event fired by backend ~3s after startup.
  const onAutoSyncComplete = useCallback(() => {
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
      .catch(() => {});
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
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Failed to save worklog");
      }
    },
    [pushToast],
  );

  // ---- Start tracking handler ---------------------------------------------
  const handlePickIssue = useCallback(
    async (issueKey: string) => {
      try {
        await startTimer(issueKey);
        navigate("/");
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Nepodařilo se spustit časomíru");
      }
    },
    [navigate, pushToast],
  );

  // Phase 18A — Item 4: start an unassigned timer.
  const handleStartUnassigned = useCallback(async () => {
    try {
      await startTimer(null);
      navigate("/");
      pushToast(
        "info",
        "Časomíra běží bez přiřazeného úkolu — nezapomeňte ho přiřadit před uložením.",
      );
    } catch (e) {
      pushToast(
        "error",
        typeof e === "string" ? e : "Nepodařilo se spustit časomíru",
      );
    }
  }, [navigate, pushToast]);

  // ---- Add entry panel -----------------------------------------------------
  const [addEntryOpen, setAddEntryOpen] = useState(false);

  // ---- Window focus -> CommandBar visibility -------------------------------
  // The bottom CommandBar should only appear while the main window has OS
  // focus; when the user switches to another app we fade it out. Listening
  // to the browser `focus`/`blur` events on `window` works equivalently
  // inside the Tauri webview because the runtime forwards OS focus changes.
  const [windowFocused, setWindowFocused] = useState(() =>
    typeof document !== "undefined" ? document.hasFocus() : true,
  );
  useEffect(() => {
    const onFocus = () => setWindowFocused(true);
    const onBlur = () => setWindowFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  // ---- Keyboard shortcuts --------------------------------------------------
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const isMac =
        typeof navigator !== "undefined" &&
        /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");
      const mod = isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
      if (!mod) return;
      const k = e.key.toLowerCase();
      if (k === ",") {
        e.preventDefault();
        navigate("/settings");
      } else if (k === "r") {
        e.preventDefault();
        void refresh();
      } else if (k === "i") {
        e.preventDefault();
        void reindex();
      } else if (k === "n") {
        e.preventDefault();
        navigate("/");
        setAddEntryOpen(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate, refresh, reindex]);

  const isSettings = location.pathname.startsWith("/settings");

  return (
    <div className="h-screen flex bg-[var(--bg-app)] text-[var(--text-primary)]">
      <IconSidebar />

      <div className="flex-1 min-w-0 flex">
        <div className="flex-1 min-w-0 flex flex-col">
          {!isSettings && (
            <div className="px-6 pt-4 pb-2">
              <StartTrackingBar
                onPickIssue={handlePickIssue}
                onStop={active ? () => setStopOpen(true) : undefined}
                onStartUnassigned={handleStartUnassigned}
              />
            </div>
          )}
          <main className="flex-1 min-w-0 overflow-y-auto">
            <Outlet
              context={{
                pushToast,
                openStopDialog: () => setStopOpen(true),
                openAddEntry: () => setAddEntryOpen(true),
              }}
            />
          </main>
          {!isSettings && (
            <div
              aria-hidden={!windowFocused}
              className="transition-opacity duration-200"
              style={{
                opacity: windowFocused ? 1 : 0,
                pointerEvents: windowFocused ? "auto" : "none",
              }}
            >
              <CommandBar
                onSettings={() => navigate("/settings")}
                onRefresh={() => void refresh()}
                onReindex={() => void reindex()}
                onNewEntry={() => setAddEntryOpen(true)}
              />
            </div>
          )}
        </div>

        <AddEntryPanel
          open={addEntryOpen}
          onClose={() => setAddEntryOpen(false)}
          onSave={async (entry) => {
            // Compose a wall-clock Date from the date + start time string.
            const [hh, mm] = entry.startTime.split(":").map((s) => parseInt(s, 10));
            const [eh, em] = entry.endTime.split(":").map((s) => parseInt(s, 10));
            const [y, mo, d] = entry.dateIso.split("-").map((s) => parseInt(s, 10));
            const startedAt = new Date(y, mo - 1, d, hh, mm, 0);
            const endedAt = new Date(y, mo - 1, d, eh, em, 0);
            const durationSeconds = Math.max(
              0,
              Math.round((endedAt.getTime() - startedAt.getTime()) / 1000),
            );
            try {
              await createManualWorklog({
                issueKey: entry.issueKey,
                startedAtMs: startedAt.getTime(),
                durationSeconds,
                comment: entry.comment.length > 0 ? entry.comment : null,
              });
              pushToast(
                "success",
                `Záznam přidán na ${entry.issueKey}.`,
              );
              queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
              queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
            } catch (e) {
              const msg = typeof e === "string" ? e : "Záznam se nepodařilo uložit";
              pushToast("error", msg);
              throw e; // Re-throw so the panel keeps the form open.
            }
          }}
        />
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

      <Toaster toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
