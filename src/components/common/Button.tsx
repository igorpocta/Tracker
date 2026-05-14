/**
 * Standard button — token-driven, no hardcoded color literals.
 *
 * Variants:
 * - `primary` — accent fill + accent-text. The single bright element in a view.
 * - `secondary` — transparent + border + primary text. The neutral default.
 * - `danger` — danger-tinted, used for destructive ops only.
 * - `ghost` — no border, no fill, just hoverable text.
 *
 * Heights are fixed (28 / 32 / 36 px) so rows of buttons align cleanly with
 * other 32px-tall controls (inputs, selects).
 */
import { clsx } from "clsx";
import type { ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const variants: Record<ButtonVariant, string> = {
  primary:
    "bg-[var(--accent)] hover:bg-[var(--accent-hover)] text-[var(--accent-text)] " +
    "disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] " +
    "focus-visible:ring-[var(--accent-ring)]",
  secondary:
    "bg-transparent border border-[var(--border-default)] hover:bg-[var(--bg-hover)] text-[var(--text-primary)] " +
    "disabled:text-[var(--text-disabled)] disabled:border-[var(--border-subtle)] " +
    "focus-visible:ring-[var(--accent-ring)]",
  danger:
    "bg-[var(--danger)] hover:brightness-110 text-white " +
    "disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] disabled:hover:brightness-100",
  ghost:
    "bg-transparent hover:bg-[var(--bg-hover)] text-[var(--text-primary)] " +
    "disabled:text-[var(--text-disabled)] " +
    "focus-visible:ring-[var(--accent-ring)]",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-7 px-2.5 text-xs",
  md: "h-8 px-3 text-xs",
  lg: "h-9 px-4 text-sm",
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
        "rounded-[var(--radius-md)] font-medium transition-colors duration-150",
        "inline-flex items-center justify-center gap-1.5",
        "disabled:cursor-not-allowed",
        "outline-none focus-visible:ring-2 ring-offset-0",
        variants[variant],
        sizes[size],
        className,
      )}
      {...rest}
    />
  );
}
