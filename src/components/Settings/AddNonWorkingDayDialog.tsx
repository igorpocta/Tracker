/**
 * Dialog for adding a new non-working day from Settings → Cíle.
 *
 *   ┌──────────────────────────────────────────┐
 *   │ Přidat nepracovní den                    │
 *   │                                          │
 *   │  Datum:    [ 2026-05-15 ]                │
 *   │  Důvod:    ( Dovolená / Svátek / Osobní) │
 *   │  Popis:    [ ... ]                       │
 *   │                                          │
 *   │              [ Zrušit ]  [ Uložit ]      │
 *   └──────────────────────────────────────────┘
 *
 * On Save the parent receives `(date, reason, label?)` so it can decide how
 * to call the backend (this keeps the dialog free of network concerns and
 * makes it trivial to test in isolation).
 */
import { useState } from "react";

import { useT } from "../../i18n";
import { formatIsoDate } from "../../lib/dates";

import type { NonWorkingReason } from "../Calendar/CellContextMenu";

export interface AddNonWorkingDayDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (date: string, reason: NonWorkingReason, label?: string) => void | Promise<void>;
}

const REASON_OPTIONS: { value: NonWorkingReason; labelKey: string }[] = [
  { value: "vacation", labelKey: "settingsGoals.reason.vacation" },
  { value: "holiday", labelKey: "settingsGoals.reason.holiday" },
  { value: "personal", labelKey: "settingsGoals.reason.personal" },
];

export function AddNonWorkingDayDialog({
  open,
  onClose,
  onSave,
}: AddNonWorkingDayDialogProps) {
  const t = useT();
  const [date, setDate] = useState<string>(() => formatIsoDate(new Date()));
  const [reason, setReason] = useState<NonWorkingReason>("vacation");
  const [label, setLabel] = useState<string>("");
  const [saving, setSaving] = useState(false);

  if (!open) return null;

  const handleSave = async () => {
    if (!date) return;
    setSaving(true);
    try {
      await onSave(date, reason, label.trim() || undefined);
      // Reset for the next invocation.
      setLabel("");
      setReason("vacation");
      setDate(formatIsoDate(new Date()));
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-label={t("settingsGoals.dialog.title")}
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="w-[360px] p-5 rounded-[var(--radius-lg)] flex flex-col gap-4"
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border-default)",
        }}
      >
        <h3 className="text-base font-semibold text-[var(--text-primary)]">
          {t("settingsGoals.dialog.title")}
        </h3>

        <label className="flex flex-col gap-1 text-sm">
          <span className="text-[var(--text-secondary)]">
            {t("settingsGoals.dialog.date")}
          </span>
          <input
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            className="px-2 h-8 rounded-[var(--radius-md)] text-sm
                       border border-[var(--border-subtle)] bg-transparent
                       text-[var(--text-primary)] outline-none
                       focus:border-[var(--accent)]"
          />
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="text-[var(--text-secondary)]">
            {t("settingsGoals.dialog.reason")}
          </span>
          <select
            value={reason}
            onChange={(e) => setReason(e.target.value as NonWorkingReason)}
            className="px-2 h-8 rounded-[var(--radius-md)] text-sm
                       border border-[var(--border-subtle)] bg-transparent
                       text-[var(--text-primary)] outline-none
                       focus:border-[var(--accent)]"
          >
            {REASON_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {t(o.labelKey)}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="text-[var(--text-secondary)]">
            {t("settingsGoals.dialog.description")}
          </span>
          <input
            type="text"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder={t("settingsGoals.dialog.descriptionPlaceholder")}
            className="px-2 h-8 rounded-[var(--radius-md)] text-sm
                       border border-[var(--border-subtle)] bg-transparent
                       text-[var(--text-primary)] outline-none
                       focus:border-[var(--accent)]"
          />
        </label>

        <div className="flex items-center justify-end gap-2 mt-1">
          <button
            type="button"
            onClick={onClose}
            disabled={saving}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       text-[var(--text-secondary)]
                       hover:bg-[var(--bg-hover)]
                       transition-colors duration-150"
          >
            {t("settingsGoals.dialog.cancel")}
          </button>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={saving || !date}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       font-semibold transition-colors duration-150
                       disabled:opacity-50"
            style={{
              background: "var(--accent)",
              color: "var(--accent-text, #fff)",
            }}
          >
            {t("settingsGoals.dialog.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
