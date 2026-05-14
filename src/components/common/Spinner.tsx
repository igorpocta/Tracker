/**
 * Lightweight CSS spinner. Avoids pulling in a dedicated icon for the busy
 * state. Render with `<Spinner aria-label="Syncing" />` for accessibility.
 */
import { clsx } from "clsx";

export interface SpinnerProps {
  /** Tailwind size class — default `w-4 h-4`. */
  className?: string;
  /** Accessible label; if omitted the spinner is treated as decorative. */
  "aria-label"?: string;
}

export function Spinner({ className, ...rest }: SpinnerProps) {
  const ariaLabel = rest["aria-label"];
  return (
    <span
      role={ariaLabel ? "status" : undefined}
      aria-label={ariaLabel}
      aria-hidden={ariaLabel ? undefined : true}
      className={clsx(
        "inline-block animate-spin rounded-full border-2 border-current border-r-transparent",
        className ?? "w-4 h-4",
      )}
    />
  );
}
