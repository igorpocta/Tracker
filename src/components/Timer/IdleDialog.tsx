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
      aria-label="Detekována ne-aktivita"
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
            Byl jsi pryč {mins} {pluralMin(mins)}
          </h3>
        </div>
        <p className="text-xs text-[var(--text-secondary)]">
          Časomíra na {issueKey || "úkolu bez přiřazení"} běžela celou dobu.
          Co s tou ne-aktivitou?
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
                Zachovat
              </span>
              <span className="block text-[11px] text-[var(--text-tertiary)]">
                Časomíra pokračuje, čas se započítá.
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
                Odečíst a zastavit
              </span>
              <span className="block text-[11px] text-[var(--text-tertiary)]">
                Uložit worklog s časem před ne-aktivitou.
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
              <span className="font-medium">Odečíst a pokračovat</span>
              <span className="block text-[11px] opacity-80">
                Zastaví, uloží, znovu spustí časomíru pro stejný úkol.
              </span>
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}

function pluralMin(n: number): string {
  if (n === 1) return "minutu";
  if (n >= 2 && n <= 4) return "minuty";
  return "minut";
}
