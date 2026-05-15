/**
 * Step 1 (multi-provider) — picker for the connection provider.
 *
 * Phase 18E: introduces multi-provider support. Active providers (Jira, Freelo)
 * are selectable; placeholders for future providers (Toggl, Clockify) are
 * greyed-out with a "Brzy" badge.
 */
import { Clock, FolderGit2, Layers, Server } from "lucide-react";

import type { ProviderKind } from "../../api/types";

export interface ProviderOption {
  kind: ProviderKind;
  label: string;
  description: string;
  available: boolean;
  Icon: typeof Server;
}

export const PROVIDERS: ProviderOption[] = [
  {
    kind: "jira",
    label: "Jira Cloud",
    description: "Atlassian Jira (REST API v3, API token)",
    available: true,
    Icon: Server,
  },
  {
    kind: "freelo",
    label: "Freelo",
    description: "Freelo (REST API v1, e-mail + API klíč)",
    available: true,
    Icon: Layers,
  },
  {
    kind: "toggl",
    label: "Toggl",
    description: "Toggl Track",
    available: false,
    Icon: Clock,
  },
  {
    kind: "clockify",
    label: "Clockify",
    description: "Clockify",
    available: false,
    Icon: FolderGit2,
  },
];

export interface StepProviderProps {
  value: ProviderKind | null;
  onChange: (next: ProviderKind) => void;
  onNext: () => void;
}

export function StepProvider({ value, onChange, onNext }: StepProviderProps) {
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (value) onNext();
      }}
      className="flex flex-col gap-4"
    >
      <div>
        <label className="text-sm font-medium text-[var(--text-primary)]">
          Vyberte poskytovatele
        </label>
        <p className="text-xs text-[var(--text-tertiary)] mt-1">
          Připojení k jednomu nebo více poskytovatelům lze přidat později v
          Nastavení.
        </p>
      </div>

      <div className="grid grid-cols-2 gap-2" role="radiogroup">
        {PROVIDERS.map((p) => {
          const Icon = p.Icon;
          const isActive = value === p.kind;
          const disabled = !p.available;
          return (
            <button
              key={p.kind}
              type="button"
              role="radio"
              aria-checked={isActive}
              disabled={disabled}
              onClick={() => onChange(p.kind)}
              data-testid={`provider-card-${p.kind}`}
              className={
                "flex flex-col items-start gap-2 p-3 rounded-[var(--radius-md)] border text-left transition-colors duration-150 " +
                (disabled
                  ? "border-dashed border-[var(--border-subtle)] text-[var(--text-tertiary)] opacity-60 cursor-not-allowed"
                  : isActive
                    ? "border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--text-primary)]"
                    : "border-[var(--border-default)] hover:bg-[var(--bg-hover)] text-[var(--text-primary)]")
              }
            >
              <div className="flex items-center gap-2 w-full">
                <Icon className="w-4 h-4 shrink-0" aria-hidden />
                <span className="text-sm font-semibold">{p.label}</span>
                {disabled && (
                  <span className="ml-auto text-[10px] px-1.5 py-0.5 rounded-full bg-[var(--bg-active)] text-[var(--text-tertiary)] uppercase tracking-wide">
                    Brzy
                  </span>
                )}
              </div>
              <p className="text-xs text-[var(--text-tertiary)]">
                {p.description}
              </p>
            </button>
          );
        })}
      </div>

      <div className="flex justify-end mt-2">
        <button
          type="submit"
          disabled={!value}
          className="h-9 px-4 rounded-[var(--radius-md)] bg-[var(--accent)] hover:bg-[var(--accent-hover)] disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] disabled:cursor-not-allowed text-[var(--accent-text)] text-sm font-medium transition-colors duration-150"
        >
          Další
        </button>
      </div>
    </form>
  );
}
