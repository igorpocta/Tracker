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
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";

import {
  createManualWorklog,
  discardTimer,
  getCacheStats,
  hasConfig,
  refreshAll,
  refreshCache,
  startTimer,
} from "../../api/commands";
import {
  invalidateAfterCacheRefresh,
  invalidateWorklogQueries,
  queryKeys,
} from "../../api/queryKeys";
import type { ActiveTimerState, WorklogRow } from "../../api/types";
import { pluralCs } from "../../lib/format";
import { clampDiscardStartMs } from "../../lib/idleGap";
import { useActivityTracker } from "../../hooks/useActivityTracker";
import { useIdleDetection } from "../../hooks/useIdleDetection";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";
import { usePomodoroTimer } from "../../hooks/usePomodoroTimer";
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
import { IdleDialog } from "../Timer/IdleDialog";
import { StopDialog } from "../Timer/TimerControls";
import { CommandBar } from "./CommandBar";
import { IconSidebar } from "./IconSidebar";
import { StartTrackingBar } from "./StartTrackingBar";
import { SyncBanner } from "./SyncBanner";

// AppShell sync — viz `refresh` níže. Šetříme API: pokud lokální cache už
// nějaké worklogy obsahuje, jedeme jen rolling 30 dní (incremental).
// Při prvním spuštění (prázdná tabulka) backend dostane `mode=full` a
// stáhne historii ~10 let zpět.

/**
 * `true` when the app is running on macOS. We use the platform sniff once at
 * module load — the result never changes during a session.
 *
 * On macOS the main window uses `titleBarStyle: "Overlay"` (see
 * `tauri.conf.json`), so our content extends behind the transparent title
 * bar. We compensate with ~28px of top padding and expose a draggable strip
 * so users can still grab the window from where the title bar visually lives.
 */
const IS_MAC =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");

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
  usePomodoroTimer();

  // Hydrate stores once.
  useEffect(() => {
    void hydrateTimer();
    void hydratePrefs();
  }, [hydrateTimer, hydratePrefs]);

  // ---- refresh -------------------------------------------------------------
  // Heuristika prvního spuštění: pokud v lokální DB nejsou žádné worklogy,
  // backend dostane `mode=full` a stáhne ~10 let historie. Při dalších
  // mountech / Cmd+R jedeme `incremental` (rolling 30 dní), což je o řád
  // levnější a stačí na běžný provoz.
  const refresh = useCallback(async () => {
    try {
      let mode: "full" | "incremental" = "incremental";
      try {
        const stats = await getCacheStats();
        if ((stats?.worklogs_local ?? 0) === 0) {
          mode = "full";
        }
      } catch {
        // Když cache_stats selže, zůstaneme u bezpečného incremental.
      }
      await refreshAll(mode);
      invalidateAfterCacheRefresh(queryClient);
    } catch {
      /* swallow — toast lives elsewhere */
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

  // Manual reindex (CommandBar / Cmd-Ctrl+I): pull the latest issues from the
  // provider, refresh the search/recent caches, and toast the outcome — a
  // silent no-op (the old behaviour) read as "the button doesn't work".
  const reindex = useCallback(async () => {
    try {
      const n = await refreshCache();
      queryClient.invalidateQueries({ queryKey: queryKeys.recentIssues.all() });
      queryClient.invalidateQueries({ queryKey: queryKeys.searchIssues.all() });
      pushToast(
        "success",
        `Reindexováno ${n} ${pluralCs(n, ["úkol", "úkoly", "úkolů"])}.`,
      );
    } catch (e) {
      pushToast("error", typeof e === "string" ? e : "Reindexace selhala.");
    }
  }, [queryClient, pushToast]);

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
      invalidateWorklogQueries(queryClient);
      setStopOpen(false);
    },
    [pushToast, queryClient],
  );
  useTauriEvent<WorklogRow>("worklog-saved", onWorklogSaved);

  // `timer-discarded` fires when the timer is dropped (no worklog written) —
  // could originate here, in the popover, or via tray. Always clear the
  // store so the running chip vanishes everywhere.
  const onTimerDiscarded = useCallback(() => {
    setActive(null);
    setStopOpen(false);
  }, [setActive]);
  useTauriEvent<boolean>("timer-discarded", onTimerDiscarded);

  // Phase 15 — mutation events. All four invalidate the same query set so
  // the visible Time Log refreshes immediately. The shared helper covers
  // both the list keys AND the derived "recently tracked" dropdowns —
  // the pre-refactor inline version only invalidated the two list keys
  // and left the dropdowns stale.
  const handleWorklogMutation = useCallback(() => {
    invalidateWorklogQueries(queryClient);
  }, [queryClient]);
  useTauriEvent<WorklogRow>("worklog-created", handleWorklogMutation);
  useTauriEvent<WorklogRow>("worklog-updated", handleWorklogMutation);
  useTauriEvent<WorklogRow>("worklog-deleted", handleWorklogMutation);
  useTauriEvent<WorklogRow>("worklog-undo-deleted", handleWorklogMutation);
  useTauriEvent<string>("worklog-delete-committed", handleWorklogMutation);
  useTauriEvent<WorklogRow>("worklog-moved", handleWorklogMutation);

  const onWorklogError = useCallback(
    (err: unknown) => {
      const msg = typeof err === "string" ? err : "Synchronizace s Jirou selhala";
      pushToast("error", `Záznam: ${msg}`);
    },
    [pushToast],
  );
  useTauriEvent<unknown>("worklog-error", onWorklogError);

  const onCacheRefreshed = useCallback(() => {
    invalidateWorklogQueries(queryClient);
  }, [queryClient]);
  useTauriEvent<number>("cache-refreshed", onCacheRefreshed);

  // Auto-sync event fired by backend ~3s after startup.
  const onAutoSyncComplete = useCallback(() => {
    invalidateWorklogQueries(queryClient);
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

  // ---- Idle detection ------------------------------------------------------
  // Pokud uživatel je pryč déle než `activity_threshold_min` a timer běží,
  // hook drží `gap` a my otevřeme dialog s Toggl-like volbami.
  const { gap: idleGap, dismiss: dismissIdleGap } = useIdleDetection();
  const handleIdleDiscard = useCallback(async () => {
    if (!idleGap) return;
    try {
      // Posunout `started_at` o délku idle dopředu, aby `stop` spočítal jen
      // skutečně pracovaný čas (před idle).
      const state = useTimerStore.getState();
      const active = state.active;
      if (active) {
        const idleMs = idleGap.returnedAtMs - idleGap.startedAtMs;
        await state.updateStart(
          clampDiscardStartMs(active.started_at, idleMs, Date.now()),
        );
      }
      await state.stop();
    } catch (e) {
      pushToast("error", typeof e === "string" ? e : "Idle: stop selhal");
    } finally {
      dismissIdleGap();
    }
  }, [idleGap, dismissIdleGap, pushToast]);

  const handleIdleDiscardContinue = useCallback(async () => {
    if (!idleGap) return;
    try {
      const state = useTimerStore.getState();
      const prev = state.active;
      const issueKey = prev?.issue_key ?? "";
      const comment = prev?.comment ?? null;
      const idleMs = idleGap.returnedAtMs - idleGap.startedAtMs;
      if (prev) {
        await state.updateStart(
          clampDiscardStartMs(prev.started_at, idleMs, Date.now()),
        );
      }
      await state.stop();
      // Hned znovu nastartuj se stejným úkolem (a komentářem) od teď.
      await startTimer(issueKey || null, Date.now(), comment);
    } catch (e) {
      pushToast(
        "error",
        typeof e === "string" ? e : "Idle: restart selhal",
      );
    } finally {
      dismissIdleGap();
    }
  }, [idleGap, dismissIdleGap, pushToast]);

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
    async (issueKey: string, comment: string) => {
      try {
        // Phase 18B — Item 6: in-flight comment threaded into start_timer.
        await startTimer(
          issueKey,
          undefined,
          comment.length > 0 ? comment : null,
        );
        navigate("/");
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Nepodařilo se spustit časomíru");
      }
    },
    [navigate, pushToast],
  );

  // Phase 18A — Item 4: start an unassigned timer.
  const handleStartUnassigned = useCallback(
    async (comment: string) => {
      try {
        await startTimer(null, undefined, comment.length > 0 ? comment : null);
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
    },
    [navigate, pushToast],
  );

  // Reassign the running timer to a different issue (RunningBar chip → picker).
  // Owns the error handling so a failed reassign surfaces a toast instead of
  // an unhandled rejection — `timerStore.assign` rethrows on failure.
  const handleReassign = useCallback(
    async (issueKey: string) => {
      try {
        await useTimerStore.getState().assign(issueKey);
        pushToast("success", `Časomíra přepnuta na ${issueKey}.`);
      } catch (e) {
        pushToast(
          "error",
          typeof e === "string" ? e : "Přepnutí úkolu selhalo.",
        );
      }
    },
    [pushToast],
  );

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
  // Cmd/Ctrl+R refresh · +I reindex · +N new entry · +, settings.
  useKeyboardShortcuts({
    onRefresh: refresh,
    onReindex: reindex,
    onNewEntry: () => {
      navigate("/");
      setAddEntryOpen(true);
    },
    onOpenSettings: () => navigate("/settings"),
  });

  const isSettings = location.pathname.startsWith("/settings");

  return (
    <div
      className={`relative h-screen flex flex-col bg-[var(--bg-app)] text-[var(--text-primary)] ${IS_MAC ? "pt-7" : ""}`}
    >
      {IS_MAC && <DragStrip />}
      <div className="flex-1 min-h-0 flex">
        <IconSidebar />

        <div className="flex-1 min-w-0 flex">
        <div className="flex-1 min-w-0 flex flex-col">
          {!isSettings && <SyncBanner />}
          {!isSettings && (
            // Phase 18B — Item 20: top toolbar sits on the same `--bg-app`
            // as the route body so there's no visible seam between the
            // tracking bar and the content below it. The single hairline at
            // the bottom hints at the boundary without being a hard divider.
            <div
              className="px-6 pt-4 pb-3"
              style={{
                background: "var(--bg-app)",
                borderBottom: "1px solid var(--border-subtle)",
              }}
            >
              <StartTrackingBar
                onPickIssue={handlePickIssue}
                onStop={active ? () => setStopOpen(true) : undefined}
                onStartUnassigned={handleStartUnassigned}
                onReassign={handleReassign}
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
            // Midnight wrap: if the end clock is strictly before the start
            // clock the user meant the entry to span past 24:00 — e.g.
            // 23:30 → 00:30 is a 1h interval, not a 23h-back-in-time one.
            // The panel's `computeDurationMinutes` already treats it this
            // way for the total-duration label; mirror the rule here so
            // `durationSeconds` matches and the backend doesn't reject
            // the row with "Trvání musí být kladné".
            if (endedAt.getTime() < startedAt.getTime()) {
              endedAt.setDate(endedAt.getDate() + 1);
            }
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
              invalidateWorklogQueries(queryClient);
            } catch (e) {
              const msg = typeof e === "string" ? e : "Záznam se nepodařilo uložit";
              pushToast("error", msg);
              throw e; // Re-throw so the panel keeps the form open.
            }
          }}
        />
        </div>
      </div>

      {active && (
        <StopDialog
          open={stopOpen}
          active={active}
          busy={timerBusy}
          onClose={() => setStopOpen(false)}
          onConfirm={handleStopConfirm}
          onDiscard={async () => {
            try {
              await discardTimer();
              // Clear the timer state in the Zustand store so the running
              // chip / Stop button disappear from the UI immediately. The
              // backend has already cleared active_timer; this just keeps
              // the React state in sync with reality.
              setActive(null);
              setStopOpen(false);
              pushToast("info", "Časomíra zahozena bez uložení.");
            } catch (e) {
              pushToast(
                "error",
                typeof e === "string" ? e : "Zahození časomíry selhalo.",
              );
            }
          }}
        />
      )}

      {idleGap && active && (
        <IdleDialog
          gap={idleGap}
          issueKey={active.issue_key}
          onKeep={dismissIdleGap}
          onDiscard={() => void handleIdleDiscard()}
          onDiscardAndContinue={() => void handleIdleDiscardContinue()}
        />
      )}

      <Toaster toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}

/**
 * macOS Overlay title-bar drag strip.
 *
 * `titleBarStyle: "Overlay"` removes the OS title bar visually; we need to
 * recreate drag-to-move ourselves. We bind React's onMouseDown directly and
 * call `Window.startDragging()` — imperative, no DOM-attribute scanning,
 * fully reliable. The module-level static import ensures the API is loaded
 * before the first click.
 *
 * Double-click se zoom-toggle řeší nativně přes `data-tauri-drag-region`
 * + macOS NSWindow titlebar — vlastní `onDoubleClick` handler vedl ke
 * dvojitému togglu (Tauri zoom + náš toggleMaximize), což na macOS bliklo
 * maximize/restore/maximize a uživatel viděl flash. Necháváme to na OS.
 *
 * Capability `core:window:allow-start-dragging` is added in
 * `src-tauri/capabilities/default.json`.
 */
function DragStrip() {
  const onMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button, a, input, textarea, select"))
      return;
    e.preventDefault();
    // Fire and forget; errors in non-Tauri contexts (tests) are silenced.
    getCurrentWindow()
      .startDragging()
      .catch(() => {
        /* noop */
      });
  }, []);

  return (
    <div
      aria-hidden
      onMouseDown={onMouseDown}
      className="fixed top-0 left-0 right-0 h-8 z-[9999]"
      data-tauri-drag-region
      style={
        {
          WebkitAppRegion: "drag",
          appRegion: "drag",
        } as React.CSSProperties
      }
    />
  );
}

