/**
 * Two-state inline confirmation button.
 *
 * Click once → switches to a "Are you sure?" pair (Confirm / Cancel) inline,
 * click again on Confirm to trigger the action. Used for destructive ops
 * like Sign out where we don't want a full dialog but do want a guard.
 */
import { clsx } from "clsx";
import { useEffect, useState } from "react";

import { Button, type ButtonVariant } from "./Button";

export interface ConfirmButtonProps {
  /** Label shown in the resting state. */
  label: string;
  /** Label shown on the Confirm button after the first click. */
  confirmLabel?: string;
  /** Label shown on the Cancel button. */
  cancelLabel?: string;
  /** Action invoked when the user confirms. */
  onConfirm: () => void | Promise<void>;
  variant?: ButtonVariant;
  /** Disables both states. */
  disabled?: boolean;
  /** Auto-cancel after this many ms in the confirmation state. */
  autoResetMs?: number;
  className?: string;
}

export function ConfirmButton({
  label,
  confirmLabel = "Potvrdit",
  cancelLabel = "Zrušit",
  onConfirm,
  variant = "danger",
  disabled = false,
  autoResetMs = 5000,
  className,
}: ConfirmButtonProps) {
  const [armed, setArmed] = useState(false);
  const [busy, setBusy] = useState(false);

  // Auto-cancel so users don't end up confirming an old prompt by accident.
  useEffect(() => {
    if (!armed) return;
    const id = window.setTimeout(() => setArmed(false), autoResetMs);
    return () => window.clearTimeout(id);
  }, [armed, autoResetMs]);

  if (!armed) {
    return (
      <Button
        variant={variant}
        size="sm"
        disabled={disabled}
        onClick={() => setArmed(true)}
        className={className}
      >
        {label}
      </Button>
    );
  }

  return (
    <div className={clsx("inline-flex items-center gap-1.5", className)}>
      <Button
        variant={variant}
        size="sm"
        disabled={disabled || busy}
        onClick={async () => {
          setBusy(true);
          try {
            await onConfirm();
            setArmed(false);
          } finally {
            setBusy(false);
          }
        }}
      >
        {confirmLabel}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        disabled={busy}
        onClick={() => setArmed(false)}
      >
        {cancelLabel}
      </Button>
    </div>
  );
}
