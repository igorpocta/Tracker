/**
 * Visual shell for the setup wizard: the progress indicator + content slot.
 * The individual step components handle their own validation and call
 * `onNext` / `onBack`.
 *
 * Phase 18E: the step list is now provider-aware. The default 3-step Jira
 * flow stays untouched; Freelo gets its own 4-step flow with a project
 * picker as the final step.
 */
import {
  Circle,
  CircleCheck,
  Globe,
  KeyRound,
  Layers,
  Mail,
  Server,
} from "lucide-react";
import type { ReactNode } from "react";

import { useT } from "../../i18n";

export interface WizardStepMeta {
  /** 1-based index used purely for accessibility. */
  index: number;
  /** i18n key for the short label shown beneath the icon. */
  label: string;
  /** lucide-react icon component for the step. */
  Icon: typeof Globe;
}

/** Step list for the Jira flow (preserved from Phase 17). */
export const JIRA_SETUP_STEPS: WizardStepMeta[] = [
  { index: 1, label: "setup.step.provider", Icon: Server },
  { index: 2, label: "setup.step.url", Icon: Globe },
  { index: 3, label: "setup.step.email", Icon: Mail },
  { index: 4, label: "setup.step.token", Icon: KeyRound },
];

/** Step list for the Freelo flow (Phase 18E). */
export const FREELO_SETUP_STEPS: WizardStepMeta[] = [
  { index: 1, label: "setup.step.provider", Icon: Server },
  { index: 2, label: "setup.step.credentials", Icon: KeyRound },
  { index: 3, label: "setup.step.projects", Icon: Layers },
];

/** Legacy alias kept so existing tests / imports continue to compile. */
export const SETUP_STEPS = JIRA_SETUP_STEPS;

export interface WizardProps {
  /** 0-based index of the active step. */
  step: number;
  /** Step list to use (defaults to the Jira flow). */
  steps?: WizardStepMeta[];
  /** Custom header label (e.g. "Připojit Freelo"). */
  title?: string;
  /** Step content. */
  children: ReactNode;
}

/**
 * Card-style wizard shell: centered, max-width ~480px, with a horizontal
 * progress indicator on top. Steps before the current one show
 * `CircleCheck`; the current step highlights its associated icon.
 */
export function Wizard({ step, steps, title, children }: WizardProps) {
  const t = useT();
  const list = steps ?? JIRA_SETUP_STEPS;
  return (
    <main className="min-h-screen flex items-center justify-center p-6">
      <section
        className="w-full max-w-[480px] bg-[var(--bg-surface)] border border-[var(--border-subtle)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] p-7"
        role="region"
        aria-label={t("setup.wizard.regionLabel")}
      >
        <header className="mb-6">
          <h1 className="text-xl font-semibold tracking-tight text-[var(--text-primary)]">
            {title ?? t("setup.title.connectAccount")}
          </h1>
          <p className="text-sm text-[var(--text-tertiary)] mt-1">
            {t("setup.wizard.stepCounter", {
              current: step + 1,
              total: list.length,
            })}
          </p>
        </header>

        <ol
          className="flex items-center justify-between mb-8"
          aria-label={t("setup.wizard.progressLabel")}
        >
          {list.map((s, i) => {
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
                    "w-9 h-9 rounded-full flex items-center justify-center transition-colors duration-150 " +
                    (isDone
                      ? "bg-[var(--accent-soft)] text-[var(--accent)]"
                      : isCurrent
                        ? "bg-[var(--accent-soft)] text-[var(--accent)] ring-2 ring-[var(--accent-ring)]"
                        : "bg-[var(--bg-active)] text-[var(--text-tertiary)]")
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
                    (isCurrent
                      ? "text-[var(--text-primary)]"
                      : "text-[var(--text-tertiary)]")
                  }
                >
                  {t(s.label)}
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
