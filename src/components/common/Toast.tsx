/**
 * Tiny in-house toast: a top-right floating pill that auto-dismisses.
 *
 * We deliberately avoid pulling in a full toast library — Phase 6 only
 * needs success/error confirmation for worklog saves.
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
  const tone =
    toast.variant === "success"
      ? "border-emerald-700/40 bg-emerald-900/30 text-emerald-100"
      : toast.variant === "error"
        ? "border-red-700/40 bg-red-900/30 text-red-100"
        : "border-neutral-800 bg-neutral-900 text-neutral-100";

  return (
    <div
      role="status"
      className={`inline-flex items-start gap-2 px-3 py-2 rounded-lg border shadow-lg ${tone}`}
    >
      <Icon
        className={`w-4 h-4 shrink-0 mt-0.5 ${
          toast.variant === "success"
            ? "text-emerald-400"
            : toast.variant === "error"
              ? "text-red-400"
              : "text-neutral-300"
        }`}
        aria-hidden
      />
      <span className="text-xs flex-1">{toast.message}</span>
      <button
        type="button"
        onClick={() => onDismiss(toast.id)}
        className="text-neutral-400 hover:text-white"
        aria-label="Dismiss"
      >
        <X className="w-3.5 h-3.5" aria-hidden />
      </button>
    </div>
  );
}
