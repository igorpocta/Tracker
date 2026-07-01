/**
 * Idle dialog — "byl jsi pryč X minut, co s tím?". Tři volby:
 *   - Zachovat (Keep): nic se neudělá, timer pokračuje.
 *   - Odečíst (Discard): timer se zastaví s časem před idle a uloží jako worklog.
 *   - Odečíst a pokračovat: stejné jako Discard + okamžitě znovu spustí
 *     timer od teď se stejným úkolem.
 */
import { Coffee, Pause, RotateCw } from "lucide-react";
import { useEffect } from "react";

import type { IdleGap } from "../../hooks/useIdleDetection";
import { useT } from "../../i18n";
import type { TFunc } from "../../i18n";

export interface IdleDialogProps {
  gap: IdleGap;
  /** Issue key aktuálního timeru (může být prázdné u unassigned). */
  issueKey: string;
  onKeep: () => void;
  onDiscard: () => void;
  onDiscardAndContinue: () => void;
}

export function IdleDialog({
  gap,
  issueKey,
  onKeep,
  onDiscard,
  onDiscardAndContinue,
}: IdleDialogProps) {
  const t = useT();
  // ESC = Keep (nejméně destruktivní volba).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onKeep();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onKeep]);

  const mins = Math.max(1, Math.round(gap.durationSeconds / 60));
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t("timer.idle.label")}
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)" }}
    >
      <div
        className="w-[460px] max-w-[92vw] p-5 rounded-[var(--radius-lg)] flex flex-col gap-3"
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border-default)",
        }}
      >
        <div className="flex items-center gap-2">
          <Coffee className="w-5 h-5 text-[var(--accent)]" aria-hidden />
          <h3 className="text-base font-semibold text-[var(--text-primary)]">
            {t("timer.idle.title", { mins, unit: pluralMin(mins, t) })}
          </h3>
        </div>
        <p className="text-xs text-[var(--text-secondary)]">
          {t("timer.idle.body", {
            issue: issueKey || t("timer.idle.noIssue"),
          })}
        </p>
        <div className="flex flex-col gap-2 mt-2">
          <button
            type="button"
            onClick={onKeep}
            className="h-9 px-3 rounded-[var(--radius-md)] text-sm text-left
                       border border-[var(--border-default)]
                       hover:bg-[var(--bg-hover)] transition-colors duration-150
                       flex items-center gap-2"
          >
            <Pause className="w-4 h-4 text-[var(--text-tertiary)]" aria-hidden />
            <span className="flex-1">
              <span className="font-medium text-[var(--text-primary)]">
                {t("timer.idle.keep")}
              </span>
              <span className="block text-[11px] text-[var(--text-tertiary)]">
                {t("timer.idle.keep.desc")}
              </span>
            </span>
          </button>
          <button
            type="button"
            onClick={onDiscard}
            className="h-9 px-3 rounded-[var(--radius-md)] text-sm text-left
                       border border-[var(--border-default)]
                       hover:bg-[var(--bg-hover)] transition-colors duration-150
                       flex items-center gap-2"
          >
            <Pause className="w-4 h-4 text-[var(--text-tertiary)]" aria-hidden />
            <span className="flex-1">
              <span className="font-medium text-[var(--text-primary)]">
                {t("timer.idle.discard")}
              </span>
              <span className="block text-[11px] text-[var(--text-tertiary)]">
                {t("timer.idle.discard.desc")}
              </span>
            </span>
          </button>
          <button
            type="button"
            onClick={onDiscardAndContinue}
            className="h-9 px-3 rounded-[var(--radius-md)] text-sm text-left
                       transition-colors duration-150 flex items-center gap-2"
            style={{
              background: "var(--accent)",
              color: "var(--accent-text, #fff)",
            }}
          >
            <RotateCw className="w-4 h-4" aria-hidden />
            <span className="flex-1">
              <span className="font-medium">{t("timer.idle.discardContinue")}</span>
              <span className="block text-[11px] opacity-80">
                {t("timer.idle.discardContinue.desc")}
              </span>
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}

function pluralMin(n: number, t: TFunc): string {
  if (n === 1) return t("timer.idle.unit.one");
  if (n >= 2 && n <= 4) return t("timer.idle.unit.few");
  return t("timer.idle.unit.many");
}
