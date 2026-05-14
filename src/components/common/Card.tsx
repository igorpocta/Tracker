/**
 * Generic neutral card with a subtle border + dark background.
 *
 * Used as the building block for most "panels" in the app — Today's timer
 * card, Reports summary cards, Settings sections, etc.
 */
import { clsx } from "clsx";
import type { HTMLAttributes, ReactNode } from "react";

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  /** Optional header content rendered above `children` with a small bottom border. */
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
        "rounded-xl border border-neutral-800/80 bg-neutral-900/60 shadow-sm",
        className,
      )}
      {...rest}
    >
      {header && (
        <div className="px-4 py-2.5 border-b border-neutral-800/70 text-xs text-neutral-300 flex items-center gap-2">
          {header}
        </div>
      )}
      <div className={paddingMap[padding]}>{children}</div>
    </div>
  );
}
