/**
 * Plain styled button used across the main app.
 *
 * Three visual variants:
 * - `primary` — the "Stop & save", "Start" call-to-action button.
 * - `secondary` — neutral cancel / dismiss buttons.
 * - `danger` — destructive operations (red). Currently unused but defined
 *   so the rest of the app can rely on a single button component.
 */
import { clsx } from "clsx";
import type { ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";
export type ButtonSize = "sm" | "md";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const variants: Record<ButtonVariant, string> = {
  primary:
    "bg-sky-600 hover:bg-sky-500 text-white disabled:bg-neutral-800 disabled:text-neutral-500",
  secondary:
    "bg-neutral-800 hover:bg-neutral-700 text-neutral-100 disabled:bg-neutral-900 disabled:text-neutral-600",
  danger:
    "bg-red-600 hover:bg-red-500 text-white disabled:bg-neutral-800 disabled:text-neutral-500",
  ghost:
    "bg-transparent hover:bg-neutral-800 text-neutral-200 disabled:text-neutral-600",
};

const sizes: Record<ButtonSize, string> = {
  sm: "px-2.5 py-1 text-xs",
  md: "px-3.5 py-2 text-sm",
};

export function Button({
  variant = "primary",
  size = "md",
  className,
  type = "button",
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <button
      // eslint-disable-next-line react/button-has-type
      type={type}
      disabled={disabled}
      className={clsx(
        "rounded-md font-medium transition-colors disabled:cursor-not-allowed inline-flex items-center justify-center gap-1.5",
        variants[variant],
        sizes[size],
        className,
      )}
      {...rest}
    />
  );
}
