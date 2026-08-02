/**
 * Shared Focus-mode session state for the surfaces that only start and stop it
 * — the sidebar button and the popover row.
 *
 * Not a zustand store on purpose: the popover runs in its own webview with its
 * own JS realm, so a store would be two independent copies pretending to be
 * one. The backend is the single source of truth and broadcasts
 * `focus-changed` to every window, which is what actually keeps the two in
 * sync.
 */
import { useCallback, useEffect, useState } from "react";

import { getFocusState, toggleFocus } from "../api/commands";
import type { FocusState } from "../api/types";
import { useTauriEvent } from "./useTauriEvent";

export interface UseFocusSession {
  /** `null` until the first fetch resolves, or in a non-Tauri context. */
  state: FocusState | null;
  active: boolean;
  /** Seconds left, or `null` for an open-ended / stopped session. */
  remainingSeconds: number | null;
  busy: boolean;
  toggle: () => Promise<void>;
}

/** Seconds left in `state`, clamped at zero. */
export function remainingSecondsOf(state: FocusState | null, nowMs: number): number | null {
  if (!state?.active || state.ends_at == null) return null;
  return Math.max(0, state.ends_at - Math.floor(nowMs / 1000));
}

export function useFocusSession(): UseFocusSession {
  const [state, setState] = useState<FocusState | null>(null);
  const [busy, setBusy] = useState(false);
  const [nowMs, setNowMs] = useState(() => Date.now());

  const refresh = useCallback(() => {
    getFocusState()
      .then(setState)
      .catch(() => {
        /* non-Tauri context (tests, web build) — stay null. */
      });
  }, []);

  useEffect(refresh, [refresh]);
  useTauriEvent<FocusState>("focus-changed", (payload) => {
    if (payload) setState(payload);
    else refresh();
  });

  // Only tick while a countdown is actually on screen.
  const counting = Boolean(state?.active && state.ends_at != null);
  useEffect(() => {
    if (!counting) return;
    const id = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [counting]);

  const toggle = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    try {
      setState(await toggleFocus());
    } catch {
      // Backend rejected or unavailable — re-read rather than guessing.
      refresh();
    } finally {
      setBusy(false);
    }
  }, [busy, refresh]);

  return {
    state,
    active: state?.active ?? false,
    remainingSeconds: remainingSecondsOf(state, nowMs),
    busy,
    toggle,
  };
}
