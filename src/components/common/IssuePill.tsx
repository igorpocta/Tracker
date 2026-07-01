/**
 * Issue key pill — small uppercase mono token with an accent outline.
 *
 *   ┌──────────┐
 *   │ DEV-792  │  ← rounded-full border, accent text
 *   └──────────┘
 *
 * By default clicking the pill opens the issue in the user's default browser
 * (Jira `<base>/browse/KEY` or `https://app.freelo.io/task/{id}` depending on
 * the key's prefix). Pass an explicit `onClick` to override, or `linkable={false}`
 * to make the pill a static span.
 */
import { clsx } from "clsx";

import { openIssue } from "../../api/commands";
import { useT } from "../../i18n";

export interface IssuePillProps {
  issueKey: string;
  /** When true, uses the secondary accent (--accent-2). */
  secondary?: boolean;
  className?: string;
  /** Custom click handler — wins over the default browser-open behaviour. */
  onClick?: () => void;
  /** Disable the default browser-open click. Useful for read-only displays. */
  linkable?: boolean;
}

export function IssuePill({
  issueKey,
  secondary = false,
  className,
  onClick,
  linkable = true,
}: IssuePillProps) {
  const t = useT();
  const color = secondary ? "var(--accent-2)" : "var(--accent)";

  // Default behaviour: open the issue in the user's default browser. The
  // backend `open_issue` command picks the right URL by provider prefix.
  const handleClick =
    onClick ??
    (linkable && issueKey
      ? () => {
          openIssue(issueKey).catch((e) => {
            console.error("[IssuePill] open_issue failed:", e);
          });
        }
      : undefined);

  const Component: "button" | "span" = handleClick ? "button" : "span";
  return (
    <Component
      type={handleClick ? "button" : undefined}
      onClick={handleClick}
      title={
        handleClick ? t("common.issue.openInBrowser", { issueKey }) : undefined
      }
      className={clsx(
        "inline-flex items-center justify-center px-2 h-6 rounded-full",
        "font-mono text-[10px] uppercase tracking-[0.08em] tabular-nums",
        "bg-transparent",
        handleClick && "hover:bg-[var(--bg-hover)] cursor-pointer",
        "transition-colors duration-150",
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
