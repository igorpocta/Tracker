/**
 * Tiny in-house toast: a top-right floating pill that auto-dismisses.
 *
 * Phase 15 — supports an optional "undo" affordance for soft-delete flows.
 * When `undo` is provided the toast shows an extra button alongside the
 * close icon; clicking it invokes `undo.action` and dismisses immediately.
 * The toast still auto-dismisses after the standard 4-5s window so the
 * undo affordance becomes unusable past that point (matches the backend's
 * 5s grace window).
 */
import { AlertCircle, CheckCircle2, X } from "lucide-react";
import { useEffect, useState } from "react";

import { useT } from "../../i18n";

export type ToastVariant = "success" | "error" | "info";

/** Optional undo affordance shown next to the close icon. */
export interface ToastUndoAction {
  /** Visible label, e.g. "Vrátit". */
  label: string;
  /** Called when the user clicks the undo affordance. */
  action: () => void;
}

export interface Toast {
  id: number;
  variant: ToastVariant;
  message: string;
  /** Optional undo affordance for soft-delete flows. */
  undo?: ToastUndoAction;
  /**
   * Time-to-live in milliseconds. Defaults to 4000. The undo affordance
   * stops being clickable once the toast auto-dismisses.
   */
  ttlMs?: number;
}

export interface ToasterProps {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

export function Toaster({ toasts, onDismiss }: ToasterProps) {
  return (
    <div
      aria-live="polite"
      className="fixed top-3 right-3 z-40 flex flex-col gap-2 max-w-sm"
    >
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} onDismiss={onDismiss} />
      ))}
    </div>
  );
}

function ToastItem({
  toast,
  onDismiss,
}: {
  toast: Toast;
  onDismiss: (id: number) => void;
}) {
  const t = useT();
  const ttl = toast.ttlMs ?? 4000;
  // Countdown for the undo affordance (e.g. "Vrátit (4)"). Updates every 1s.
  const [secondsLeft, setSecondsLeft] = useState(Math.ceil(ttl / 1000));

  useEffect(() => {
    const dismiss = window.setTimeout(() => onDismiss(toast.id), ttl);
    let tick: number | undefined;
    if (toast.undo) {
      tick = window.setInterval(() => {
        setSecondsLeft((s) => Math.max(0, s - 1));
      }, 1000);
    }
    return () => {
      window.clearTimeout(dismiss);
      if (tick) window.clearInterval(tick);
    };
  }, [toast.id, toast.undo, ttl, onDismiss]);

  const Icon =
    toast.variant === "success"
      ? CheckCircle2
      : toast.variant === "error"
        ? AlertCircle
        : CheckCircle2;

  const iconColor =
    toast.variant === "success"
      ? "text-[var(--success)]"
      : toast.variant === "error"
        ? "text-[var(--danger)]"
        : "text-[var(--text-secondary)]";

  return (
    <div
      role="status"
      className="inline-flex items-center gap-2 px-3 py-2 rounded-[var(--radius-md)]
                 bg-[var(--bg-elevated)] border border-[var(--border-default)]
                 text-[var(--text-primary)] shadow-[var(--shadow-md)]"
    >
      <Icon
        className={`w-4 h-4 shrink-0 ${iconColor}`}
        aria-hidden
      />
      <span className="text-xs flex-1">{toast.message}</span>
      {toast.undo && secondsLeft > 0 && (
        <button
          type="button"
          onClick={() => {
            toast.undo!.action();
            onDismiss(toast.id);
          }}
          className="text-[11px] font-medium px-2 py-0.5 rounded-[var(--radius-sm)]
                     text-[var(--accent)] hover:bg-[var(--bg-hover)]
                     transition-colors duration-150"
        >
          {toast.undo.label} ({secondsLeft})
        </button>
      )}
      <button
        type="button"
        onClick={() => onDismiss(toast.id)}
        className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
        aria-label={t("common.toast.close")}
      >
        <X className="w-3.5 h-3.5" aria-hidden />
      </button>
    </div>
  );
}
