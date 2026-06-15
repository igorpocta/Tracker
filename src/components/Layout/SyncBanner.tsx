/**
 * Sync progress banner.
 *
 * Naslouchá backend eventu `auto-sync-progress` (manual i auto sync) a
 * vykresluje 3-fázový indikátor: connection → issues → worklogs. Po
 * `auto-sync-complete` chvíli zobrazí souhrn a pak zmizí.
 */
import { AlertTriangle, Check, Loader2, Link2, ListTodo, Clock, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { SyncRunStatus } from "../../api/types";
import { useTauriEvent } from "../../hooks/useTauriEvent";

type Phase = "connection" | "issues" | "worklogs";

interface ProgressPayload {
  phase: Phase | "starting" | "done";
  current: number; // 1-based
  total: number;
  connection_id?: number | null;
  connection_name?: string | null;
  provider?: "jira" | "freelo" | null;
  /** Provider count once the phase finishes; `null` while in progress. */
  count?: number | null;
  /** Set when the phase failed; subsequent phases for this connection are skipped. */
  error?: string | null;
}

interface PhaseState {
  status: "pending" | "active" | "done" | "error";
  count: number | null;
  error: string | null;
}

interface BannerState {
  visible: boolean;
  /** P2-2: when true the banner stays until the user dismisses it (failures). */
  sticky: boolean;
  finalIssues: number | null;
  finalWorklogs: number | null;
  connectionName: string | null;
  provider: "jira" | "freelo" | null;
  current: number;
  total: number;
  phases: Record<Phase, PhaseState>;
}

const INITIAL_PHASES: Record<Phase, PhaseState> = {
  connection: { status: "pending", count: null, error: null },
  issues: { status: "pending", count: null, error: null },
  worklogs: { status: "pending", count: null, error: null },
};

const INITIAL_BANNER_STATE: BannerState = {
  visible: false,
  sticky: false,
  finalIssues: null,
  finalWorklogs: null,
  connectionName: null,
  provider: null,
  current: 0,
  total: 0,
  phases: INITIAL_PHASES,
};

const PHASE_ORDER: Phase[] = ["connection", "issues", "worklogs"];

const PHASE_META: Record<Phase, { label: string; icon: typeof Link2 }> = {
  connection: { label: "Připojuji se", icon: Link2 },
  issues: { label: "Načítám úkoly", icon: ListTodo },
  worklogs: { label: "Načítám záznamy", icon: Clock },
};

const PROVIDER_LABEL: Record<"jira" | "freelo", string> = {
  jira: "Jira",
  freelo: "Freelo",
};

export function SyncBanner() {
  const [state, setState] = useState<BannerState>(INITIAL_BANNER_STATE);
  const hideTimer = useRef<number | null>(null);

  useTauriEvent<ProgressPayload>("auto-sync-progress", (payload) => {
    if (hideTimer.current !== null) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    setState((prev) => applyProgress(prev, payload));
  });

  useTauriEvent<{
    issues?: number;
    worklogs?: number;
    status?: SyncRunStatus;
  } | null>("auto-sync-complete", (payload) => {
    // P2-2: a partial/full failure must stay on screen until the user
    // dismisses it — only a clean success auto-hides. We also fall back to
    // the per-phase error states so an old backend without `status` still
    // keeps failures visible.
    const failed = payload?.status === "failed" || payload?.status === "partial";
    setState((prev) => {
      const hasPhaseError = PHASE_ORDER.some(
        (p) => prev.phases[p].status === "error",
      );
      return {
        ...prev,
        visible: true,
        sticky: failed || hasPhaseError,
        finalIssues: payload?.issues ?? null,
        finalWorklogs: payload?.worklogs ?? null,
        phases: {
          // Keep error states visible — `done` shouldn't overwrite a failure.
          connection: finalizePhase(prev.phases.connection),
          issues: finalizePhase(prev.phases.issues),
          worklogs: finalizePhase(prev.phases.worklogs),
        },
      };
    });
    if (hideTimer.current !== null) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    // Don't schedule the auto-hide when the run wasn't a clean success.
    if (failed) return;
    hideTimer.current = window.setTimeout(() => {
      setState((prev) => {
        // A late per-phase error still pins the banner open.
        if (PHASE_ORDER.some((p) => prev.phases[p].status === "error")) {
          return { ...prev, sticky: true };
        }
        return { ...prev, visible: false };
      });
      hideTimer.current = window.setTimeout(() => {
        setState((prev) =>
          prev.sticky ? prev : { ...INITIAL_BANNER_STATE },
        );
      }, 300);
    }, 1800);
  });

  const dismiss = useCallback(() => {
    if (hideTimer.current !== null) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    setState({ ...INITIAL_BANNER_STATE });
  }, []);

  useEffect(
    () => () => {
      if (hideTimer.current !== null) {
        window.clearTimeout(hideTimer.current);
      }
    },
    [],
  );

  if (!state.visible) return null;

  const hasError = PHASE_ORDER.some((p) => state.phases[p].status === "error");
  const isComplete =
    !hasError &&
    state.phases.worklogs.status === "done" &&
    state.phases.issues.status === "done";

  return (
    <div
      role="status"
      aria-live="polite"
      className="transition-opacity duration-200 px-4 py-2 border-b text-xs"
      style={{
        opacity: state.visible ? 1 : 0,
        background: "var(--bg-surface)",
        borderColor: "var(--border-subtle)",
        color: "var(--text-secondary)",
      }}
    >
      <div className="flex items-center gap-4 flex-wrap">
        <div className="flex items-center gap-2 min-w-0">
          <span className="font-medium text-[var(--text-primary)] truncate">
            {hasError
              ? "Synchronizace selhala"
              : isComplete
                ? "Synchronizace dokončena"
                : state.connectionName
                  ? `Synchronizuji ${state.connectionName}`
                  : "Synchronizuji…"}
          </span>
          {state.total > 1 && !isComplete && (
            <span className="font-mono tabular-nums text-[var(--text-tertiary)]">
              {state.current}/{state.total}
            </span>
          )}
          {state.provider && !isComplete && (
            <span
              className="px-1.5 py-0.5 rounded-full text-[10px] uppercase tracking-wider"
              style={{
                background: "var(--accent-soft)",
                color: "var(--accent)",
              }}
            >
              {PROVIDER_LABEL[state.provider]}
            </span>
          )}
        </div>

        <div className="flex items-center gap-1.5 flex-wrap ml-auto">
          {PHASE_ORDER.map((phase, idx) => {
            const meta = PHASE_META[phase];
            const ph = state.phases[phase];
            const Icon =
              ph.status === "error"
                ? AlertTriangle
                : ph.status === "done"
                  ? Check
                  : ph.status === "active"
                    ? Loader2
                    : meta.icon;
            const color =
              ph.status === "error"
                ? "var(--danger, #c0392b)"
                : ph.status === "done" || ph.status === "active"
                  ? "var(--accent)"
                  : "var(--text-tertiary)";
            return (
              <div key={phase} className="flex items-center gap-1.5">
                {idx > 0 && (
                  <span
                    className="w-3 h-px"
                    style={{ background: "var(--border-subtle)" }}
                    aria-hidden
                  />
                )}
                <div
                  className="flex items-center gap-1 px-1.5 py-0.5 rounded"
                  style={{ color }}
                  title={ph.error ?? undefined}
                >
                  <Icon
                    className={`w-3.5 h-3.5 ${ph.status === "active" ? "animate-spin" : ""}`}
                    aria-hidden
                  />
                  <span>{meta.label}</span>
                  {ph.count !== null && (
                    <span className="font-mono tabular-nums">{ph.count}</span>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {(state.sticky || hasError) && (
          <button
            type="button"
            onClick={dismiss}
            aria-label="Zavřít"
            title="Zavřít"
            className="ml-1 p-1 rounded hover:bg-[var(--bg-hover)]
                       text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                       transition-colors duration-150 shrink-0"
          >
            <X className="w-3.5 h-3.5" aria-hidden />
          </button>
        )}
      </div>
    </div>
  );
}

function applyProgress(
  prev: BannerState,
  p: ProgressPayload,
): BannerState {
  // Backwards compat: an old "starting" frame just turns the banner on.
  // A fresh run clears any previous sticky failure (P2-2).
  if (p.phase === "starting") {
    return { ...prev, visible: true, sticky: false, total: p.total };
  }
  if (p.phase === "done") {
    return prev;
  }
  const phaseKey = p.phase;
  const next: Record<Phase, PhaseState> = { ...prev.phases };
  const phaseIdx = PHASE_ORDER.indexOf(phaseKey);

  // Cokoli před aktuální fází mělo proběhnout — pokud to nezhavarovalo,
  // promoťuj na "done".
  for (let i = 0; i < phaseIdx; i++) {
    const key = PHASE_ORDER[i];
    if (next[key].status !== "error") {
      next[key] = { ...next[key], status: "done" };
    }
  }

  // Současná fáze: error má přednost, jinak count ⇒ done, jinak active.
  if (p.error) {
    next[phaseKey] = {
      status: "error",
      count: next[phaseKey].count,
      error: p.error,
    };
  } else if (p.count != null) {
    next[phaseKey] = {
      status: "done",
      count: p.count,
      error: null,
    };
  } else {
    next[phaseKey] = {
      status: "active",
      count: next[phaseKey].count,
      error: null,
    };
  }

  // Při přechodu na další connection resetni fáze pro tu novou.
  const isNewConnection =
    prev.current > 0 && p.current !== prev.current && phaseKey === "connection";
  if (isNewConnection) {
    next.issues = { status: "pending", count: null, error: null };
    next.worklogs = { status: "pending", count: null, error: null };
  }

  return {
    ...prev,
    visible: true,
    sticky: false,
    connectionName: p.connection_name ?? prev.connectionName,
    provider: p.provider ?? prev.provider,
    current: p.current ?? prev.current,
    total: p.total ?? prev.total,
    phases: next,
  };
}

/** Při `auto-sync-complete` ošli vše dokončené, ale nešahej na error. */
function finalizePhase(p: PhaseState): PhaseState {
  if (p.status === "error") return p;
  return { ...p, status: "done" };
}
