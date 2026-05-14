/**
 * Visual shell for the setup wizard: the progress indicator + content slot.
 * The individual step components handle their own validation and call
 * `onNext` / `onBack`.
 */
import { Circle, CircleCheck, Globe, KeyRound, Mail } from "lucide-react";
import type { ReactNode } from "react";

export interface WizardStepMeta {
  /** 1-based index used purely for accessibility. */
  index: number;
  /** Short label shown beneath the icon. */
  label: string;
  /** lucide-react icon component for the step. */
  Icon: typeof Globe;
}

export const SETUP_STEPS: WizardStepMeta[] = [
  { index: 1, label: "URL", Icon: Globe },
  { index: 2, label: "Email", Icon: Mail },
  { index: 3, label: "Token", Icon: KeyRound },
];

export interface WizardProps {
  /** 0-based index of the active step. */
  step: number;
  /** Step content. */
  children: ReactNode;
}

/**
 * Card-style wizard shell: centered, max-width ~480px, with a horizontal
 * progress indicator on top. Steps before the current one show
 * `CircleCheck`; the current step highlights its associated icon.
 */
export function Wizard({ step, children }: WizardProps) {
  return (
    <main className="min-h-screen flex items-center justify-center p-6">
      <section
        className="w-full max-w-[480px] bg-neutral-900/80 border border-neutral-800 rounded-2xl shadow-xl p-6"
        role="region"
        aria-label="Setup wizard"
      >
        <header className="mb-6">
          <h1 className="text-xl font-semibold tracking-tight">Connect to Jira</h1>
          <p className="text-sm text-neutral-400 mt-1">
            Step {step + 1} of {SETUP_STEPS.length}
          </p>
        </header>

        <ol
          className="flex items-center justify-between mb-8"
          aria-label="Setup progress"
        >
          {SETUP_STEPS.map((s, i) => {
            const isDone = i < step;
            const isCurrent = i === step;
            const StepIcon = s.Icon;
            return (
              <li
                key={s.index}
                className="flex flex-col items-center gap-1.5 flex-1"
                aria-current={isCurrent ? "step" : undefined}
              >
                <div
                  className={
                    "w-9 h-9 rounded-full flex items-center justify-center transition-colors " +
                    (isDone
                      ? "bg-emerald-600/20 text-emerald-400"
                      : isCurrent
                        ? "bg-sky-600/20 text-sky-300 ring-2 ring-sky-500/40"
                        : "bg-neutral-800 text-neutral-500")
                  }
                >
                  {isDone ? (
                    <CircleCheck className="w-5 h-5" aria-hidden />
                  ) : isCurrent ? (
                    <StepIcon className="w-5 h-5" aria-hidden />
                  ) : (
                    <Circle className="w-5 h-5" aria-hidden />
                  )}
                </div>
                <span
                  className={
                    "text-xs " +
                    (isCurrent ? "text-white" : "text-neutral-500")
                  }
                >
                  {s.label}
                </span>
              </li>
            );
          })}
        </ol>

        {children}
      </section>
    </main>
  );
}
