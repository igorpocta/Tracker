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
        "inline-flex items-center justify-center rounded-md w-8 h-8 transition-colors disabled:cursor-not-allowed disabled:opacity-50",
        filled
          ? "bg-neutral-800 hover:bg-neutral-700 text-neutral-100"
          : "text-neutral-400 hover:text-neutral-100 hover:bg-neutral-800",
        className,
      )}
      {...rest}
    />
  );
}
