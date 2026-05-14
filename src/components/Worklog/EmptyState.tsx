/**
 * Friendly empty-state placeholder used across worklog lists.
 *
 * Uses an inline SVG illustration so we don't have to ship a separate asset.
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
        "flex flex-col items-center justify-center text-center py-10 px-6 text-neutral-400 gap-3",
        className,
      )}
    >
      <svg
        aria-hidden
        viewBox="0 0 64 64"
        width="56"
        height="56"
        className="text-neutral-700"
      >
        <circle cx="32" cy="32" r="28" fill="currentColor" opacity="0.25" />
        <circle
          cx="32"
          cy="32"
          r="20"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        />
        <path
          d="M32 18v14l10 6"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          fill="none"
        />
      </svg>
      <div>
        <h3 className="text-sm font-medium text-neutral-200">{title}</h3>
        {description && (
          <p className="text-xs text-neutral-500 mt-1 max-w-sm mx-auto">
            {description}
          </p>
        )}
      </div>
      {children}
    </div>
  );
}
