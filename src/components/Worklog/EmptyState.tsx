/**
 * Friendly empty-state placeholder used across worklog lists.
 *
 * Restrained: a small clock-face glyph, a single line of text, optional CTA.
 */
import { clsx } from "clsx";
import type { ReactNode } from "react";

export interface EmptyStateProps {
  title: string;
  description?: ReactNode;
  /** Optional CTA button(s). */
  children?: ReactNode;
  className?: string;
}

export function EmptyState({
  title,
  description,
  children,
  className,
}: EmptyStateProps) {
  return (
    <div
      className={clsx(
        "flex flex-col items-center justify-center text-center py-10 px-6 gap-3",
        className,
      )}
    >
      <svg
        aria-hidden
        viewBox="0 0 64 64"
        width="40"
        height="40"
        className="text-[var(--text-disabled)]"
      >
        <circle
          cx="32"
          cy="32"
          r="20"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        />
        <path
          d="M32 20v12l8 5"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          fill="none"
        />
      </svg>
      <div>
        <h3 className="text-sm font-medium text-[var(--text-primary)]">{title}</h3>
        {description && (
          <p className="text-xs text-[var(--text-tertiary)] mt-1 max-w-sm mx-auto">
            {description}
          </p>
        )}
      </div>
      {children}
    </div>
  );
}
