/**
 * Neutral surface card. Used for every "panel" in the app — Today's timer,
 * Reports summary cards, Settings sections, etc.
 *
 * Surfaces follow the dark/light token system. A hairline border anchors
 * the card without competing visually with its content.
 */
import { clsx } from "clsx";
import type { HTMLAttributes, ReactNode } from "react";

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  /** Optional header content rendered above `children` with a subtle bottom border. */
  header?: ReactNode;
  /** Tweaks the inner padding — `none` is useful when the body is its own list. */
  padding?: "none" | "sm" | "md" | "lg";
}

const paddingMap = {
  none: "",
  sm: "p-3",
  md: "p-4",
  lg: "p-5",
};

export function Card({
  header,
  padding = "md",
  className,
  children,
  ...rest
}: CardProps) {
  return (
    <div
      className={clsx(
        "rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-surface)]",
        "shadow-[var(--shadow-sm)]",
        className,
      )}
      {...rest}
    >
      {header && (
        <div className="px-4 py-2.5 border-b border-[var(--border-subtle)] text-xs text-[var(--text-secondary)] flex items-center gap-2">
          {header}
        </div>
      )}
      <div className={paddingMap[padding]}>{children}</div>
    </div>
  );
}
