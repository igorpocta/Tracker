/**
 * Tiny in-house toast: a top-right floating pill that auto-dismisses.
 */
import { AlertCircle, CheckCircle2, X } from "lucide-react";
import { useEffect } from "react";

export type ToastVariant = "success" | "error" | "info";

export interface Toast {
  id: number;
  variant: ToastVariant;
  message: string;
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
  useEffect(() => {
    const id = window.setTimeout(() => onDismiss(toast.id), 4000);
    return () => window.clearTimeout(id);
  }, [toast.id, onDismiss]);

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
      className="inline-flex items-start gap-2 px-3 py-2 rounded-[var(--radius-md)]
                 bg-[var(--bg-elevated)] border border-[var(--border-default)]
                 text-[var(--text-primary)] shadow-[var(--shadow-md)]"
    >
      <Icon
        className={`w-4 h-4 shrink-0 mt-0.5 ${iconColor}`}
        aria-hidden
      />
      <span className="text-xs flex-1">{toast.message}</span>
      <button
        type="button"
        onClick={() => onDismiss(toast.id)}
        className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
        aria-label="Dismiss"
      >
        <X className="w-3.5 h-3.5" aria-hidden />
      </button>
    </div>
  );
}
