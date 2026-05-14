/**
 * Settings → Extensions.
 *
 * Placeholder section. The original product surfaces browser-extension
 * integration here; we leave the slot in place but report "no extensions"
 * so the navigation feels complete.
 */
import { Puzzle } from "lucide-react";

export default function Extensions() {
  return (
    <div className="flex flex-col gap-6 max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Extensions
        </h2>
      </header>

      <div
        className="rounded-[var(--radius-lg)] p-8 text-center
                   border border-dashed border-[var(--border-subtle)]"
      >
        <Puzzle
          className="w-8 h-8 mx-auto text-[var(--text-tertiary)]"
          aria-hidden
        />
        <h3 className="mt-3 text-sm font-medium text-[var(--text-primary)]">
          No extensions installed
        </h3>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-1">
          When you install browser or editor companions, they'll appear here.
        </p>
      </div>
    </div>
  );
}
