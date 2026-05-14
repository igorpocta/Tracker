/**
 * Non-blocking auto-sync banner — Phase 18B Item 19.
 *
 * Subscribes to the backend `auto-sync-progress` events and renders a
 * compact status pill at the top of the route while a sync is in flight.
 * Fades out shortly after `auto-sync-complete` fires.
 */
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import { useTauriEvent } from "../../hooks/useTauriEvent";

type Phase = "starting" | "issues" | "worklogs" | "done";

interface ProgressPayload {
  phase: Phase;
  current: number;
  total: number;
}

const PHASE_LABEL: Record<Phase, string> = {
  starting: "Synchronizuji s Jirou…",
  issues: "Načítám úkoly…",
  worklogs: "Načítám záznamy…",
  done: "Hotovo",
};

export function SyncBanner() {
  const [progress, setProgress] = useState<ProgressPayload | null>(null);
  const [visible, setVisible] = useState(false);

  useTauriEvent<ProgressPayload>("auto-sync-progress", (payload) => {
    setProgress(payload);
    setVisible(true);
  });
  useTauriEvent<unknown>("auto-sync-complete", () => {
    setProgress((p) => (p ? { ...p, phase: "done" } : null));
    // Fade out shortly after.
    window.setTimeout(() => {
      setVisible(false);
      window.setTimeout(() => setProgress(null), 300);
    }, 800);
  });

  useEffect(() => {
    if (!progress) setVisible(false);
  }, [progress]);

  if (!progress) return null;

  const label = PHASE_LABEL[progress.phase] ?? "Synchronizuji…";
  const counter =
    progress.total > 1
      ? ` (${Math.min(progress.current + 1, progress.total)}/${progress.total})`
      : "";

  return (
    <div
      role="status"
      aria-live="polite"
      className="transition-opacity duration-200 px-3 py-1.5 flex items-center gap-2
                 text-xs"
      style={{
        opacity: visible ? 1 : 0,
        background: "var(--accent-soft)",
        color: "var(--accent)",
        borderBottom: "1px solid var(--accent-soft)",
      }}
    >
      <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden />
      <span>
        {label}
        {counter}
      </span>
    </div>
  );
}
