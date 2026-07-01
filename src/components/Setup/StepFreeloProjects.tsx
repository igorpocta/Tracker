/**
 * Step 3 (Freelo) — pick which projects to sync into the issues cache.
 *
 * The picker is required: at least one project must be selected before the
 * "Dokončit" button enables. The user can change this later in Settings →
 * Připojení → Freelo.
 */
import { CheckSquare, LoaderCircle, Square } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { FreeloProjectDto } from "../../api/types";
import { useT } from "../../i18n";

export interface StepFreeloProjectsProps {
  /** Fetch fn injected so the step can be exercised with a Promise mock. */
  fetchProjects: () => Promise<FreeloProjectDto[]>;
  /** Called when "Dokončit" is clicked with the final selection. */
  onFinish: (projectIds: number[]) => void | Promise<void>;
  onBack: () => void;
}

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; projects: FreeloProjectDto[] }
  | { kind: "error"; message: string };

export function StepFreeloProjects({
  fetchProjects,
  onFinish,
  onBack,
}: StepFreeloProjectsProps) {
  const t = useT();
  const [load, setLoad] = useState<LoadState>({ kind: "loading" });
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [filter, setFilter] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetchProjects()
      .then((projects) => {
        if (cancelled) return;
        setLoad({ kind: "ok", projects });
        const preset = new Set(
          projects.filter((p) => p.selected).map((p) => p.id),
        );
        setSelected(preset);
      })
      .catch((e) => {
        if (cancelled) return;
        const message =
          typeof e === "string"
            ? e
            : e instanceof Error
              ? e.message
              : t("setup.projects.loadError");
        setLoad({ kind: "error", message });
      });
    return () => {
      cancelled = true;
    };
  }, [fetchProjects, t]);

  const filteredProjects = useMemo(() => {
    if (load.kind !== "ok") return [];
    const needle = filter.trim().toLowerCase();
    if (!needle) return load.projects;
    return load.projects.filter((p) =>
      p.name.toLowerCase().includes(needle),
    );
  }, [filter, load]);

  function toggle(id: number) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  async function handleFinish(e: React.FormEvent) {
    e.preventDefault();
    if (selected.size === 0 || submitting) return;
    setSubmitting(true);
    try {
      await onFinish(Array.from(selected));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleFinish} className="flex flex-col gap-4">
      <div>
        <label className="text-sm font-medium text-[var(--text-primary)]">
          {t("setup.projects.label")}
        </label>
        <p className="text-xs text-[var(--text-tertiary)] mt-1">
          {t("setup.projects.hint")}
        </p>
      </div>

      {load.kind === "loading" && (
        <div
          className="flex items-center gap-2 text-sm text-[var(--text-tertiary)]"
          role="status"
        >
          <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          {t("setup.projects.loading")}
        </div>
      )}

      {load.kind === "error" && (
        <p className="text-xs text-[var(--danger)]" role="alert">
          {load.message}
        </p>
      )}

      {load.kind === "ok" && load.projects.length === 0 && (
        <p className="text-sm text-[var(--text-tertiary)]">
          {t("setup.projects.empty")}
        </p>
      )}

      {load.kind === "ok" && load.projects.length > 0 && (
        <>
          <input
            type="text"
            placeholder={t("setup.projects.searchPlaceholder")}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm text-[var(--text-primary)] transition-colors duration-150"
          />
          <ul
            className="max-h-[260px] overflow-y-auto flex flex-col gap-1 border border-[var(--border-subtle)] rounded-[var(--radius-md)] p-2"
            data-testid="freelo-projects-list"
          >
            {filteredProjects.map((p) => {
              const isChecked = selected.has(p.id);
              return (
                <li key={p.id}>
                  <button
                    type="button"
                    onClick={() => toggle(p.id)}
                    aria-pressed={isChecked}
                    data-testid={`freelo-project-${p.id}`}
                    className={
                      "w-full text-left flex items-center gap-2 px-2 py-1.5 rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] transition-colors duration-150 " +
                      (isChecked ? "text-[var(--text-primary)]" : "text-[var(--text-secondary)]")
                    }
                  >
                    {isChecked ? (
                      <CheckSquare
                        className="w-4 h-4 text-[var(--accent)]"
                        aria-hidden
                      />
                    ) : (
                      <Square
                        className="w-4 h-4 text-[var(--text-tertiary)]"
                        aria-hidden
                      />
                    )}
                    <span className="text-sm flex-1 truncate">{p.name}</span>
                    {p.state !== "active" && (
                      <span className="text-[10px] uppercase text-[var(--text-tertiary)] tracking-wide">
                        {p.state}
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
          <p className="text-xs text-[var(--text-tertiary)]">
            {t("setup.projects.selectedCount", {
              selected: selected.size,
              total: load.projects.length,
            })}
          </p>
        </>
      )}

      <div className="flex justify-between mt-2">
        <button
          type="button"
          onClick={onBack}
          className="h-9 px-4 rounded-[var(--radius-md)] border border-[var(--border-default)] hover:bg-[var(--bg-hover)] text-sm font-medium text-[var(--text-primary)] transition-colors duration-150"
        >
          {t("setup.button.back")}
        </button>
        <button
          type="submit"
          disabled={selected.size === 0 || submitting}
          className="h-9 px-4 rounded-[var(--radius-md)] bg-[var(--accent)] hover:bg-[var(--accent-hover)] disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] disabled:cursor-not-allowed text-[var(--accent-text)] text-sm font-medium transition-colors duration-150 flex items-center gap-2"
        >
          {submitting && (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          )}
          {t("setup.button.finish")}
        </button>
      </div>
    </form>
  );
}
