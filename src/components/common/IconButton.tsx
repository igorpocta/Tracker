/**
 * Square icon-only button. The `aria-label` prop is required (icons aren't
 * self-describing for screen readers).
 */
import { clsx } from "clsx";
import type { ButtonHTMLAttributes, ReactNode } from "react";

export interface IconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  /** Required accessible label; the icon alone is not enough. */
  "aria-label": string;
  /** Lucide icon element. */
  children: ReactNode;
  /** Render with a subtle background even when not hovered. */
  filled?: boolean;
}

export function IconButton({
  className,
  filled = false,
  type = "button",
  ...rest
}: IconButtonProps) {
  return (
    <button
      // eslint-disable-next-line react/button-has-type
      type={type}
      className={clsx(
        "inline-flex items-center justify-center rounded-[var(--radius-md)] w-8 h-8",
        "transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50",
        "outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent-ring)]",
        filled
          ? "bg-[var(--bg-hover)] hover:bg-[var(--bg-active)] text-[var(--text-primary)]"
          : "text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]",
        className,
      )}
      {...rest}
    />
  );
}
