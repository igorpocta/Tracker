/**
 * A settings "tile" — one logical group of options in its own bordered,
 * surface-filled card. Replaces the old flat `<section>`s that sat directly on
 * the page background and blurred together into one grey block.
 *
 * Visual hierarchy: the page is `--bg-app` (grey), each tile is `--bg-surface`
 * (white) with a subtle border + shadow so groups read as distinct panels;
 * controls inside a tile use `--bg-app` again for contrast.
 */
import type { ReactNode } from "react";

export function SettingsCard({
  title,
  description,
  action,
  children,
}: {
  /** Header title. Omit to wrap a self-titled component in bare tile chrome. */
  title?: string;
  description?: string;
  /** Optional control aligned to the right of the header (e.g. a toggle). */
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section
      className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] shadow-[var(--shadow-sm)] p-5"
    >
      {title && (
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-[var(--text-primary)]">
              {title}
            </h3>
            {description && (
              <p className="text-[12px] leading-relaxed text-[var(--text-tertiary)] mt-1">
                {description}
              </p>
            )}
          </div>
          {action && <div className="shrink-0">{action}</div>}
        </div>
      )}
      <div className={title ? "mt-4" : undefined}>{children}</div>
    </section>
  );
}
