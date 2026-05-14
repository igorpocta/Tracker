/**
 * Issue key pill — small uppercase mono token with an accent outline.
 *
 *   ┌──────────┐
 *   │ DEV-792  │  ← rounded-full border, accent text
 *   └──────────┘
 */
import { clsx } from "clsx";

export interface IssuePillProps {
  issueKey: string;
  /** When true, uses the secondary accent (--accent-2). */
  secondary?: boolean;
  className?: string;
  onClick?: () => void;
}

export function IssuePill({
  issueKey,
  secondary = false,
  className,
  onClick,
}: IssuePillProps) {
  const color = secondary ? "var(--accent-2)" : "var(--accent)";
  const Component: "button" | "span" = onClick ? "button" : "span";
  return (
    <Component
      type={onClick ? "button" : undefined}
      onClick={onClick}
      className={clsx(
        "inline-flex items-center justify-center px-2 h-6 rounded-full",
        "font-mono text-[10px] uppercase tracking-[0.08em] tabular-nums",
        "bg-transparent",
        className,
      )}
      style={{
        color,
        border: `1px solid ${color}`,
      }}
    >
      {issueKey}
    </Component>
  );
}
