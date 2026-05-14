/**
 * Bottom command bar — keyboard shortcut hint strip.
 *
 * Reference: `screens/SCR-20260514-rjbm-2.png`.
 *
 *   ┌─────────────────────────────────────────────────────────┐
 *   │  ⌘, Settings  ⌘R Refresh  ⌘I Re-index  ⌘N New entry    │
 *   └─────────────────────────────────────────────────────────┘
 *
 * Renders a centered pill with 4 shortcut chips. Mac users see `⌘`; other
 * platforms see `Ctrl`. The chips are non-interactive — they're affordances
 * that hint at the keyboard shortcuts that are wired up at the AppShell
 * level. Hovering doesn't do anything; this is intentionally restrained.
 */

interface Chip {
  keys: string;
  label: string;
}

export interface CommandBarProps {
  onSettings?: () => void;
  onRefresh?: () => void;
  onReindex?: () => void;
  onNewEntry?: () => void;
}

export function CommandBar({
  onSettings,
  onRefresh,
  onReindex,
  onNewEntry,
}: CommandBarProps) {
  const isMac =
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");
  const mod = isMac ? "⌘" : "Ctrl";

  const chips: Array<Chip & { onClick?: () => void }> = [
    { keys: `${mod},`, label: "Nastavení", onClick: onSettings },
    { keys: `${mod}R`, label: "Obnovit", onClick: onRefresh },
    { keys: `${mod}I`, label: "Reindexovat", onClick: onReindex },
    { keys: `${mod}N`, label: "Nový záznam", onClick: onNewEntry },
  ];

  return (
    <div className="flex justify-center px-6 py-3">
      <div
        className="inline-flex items-center gap-4 px-3 h-7 rounded-full
                   border border-[var(--border-subtle)] text-[11px]
                   text-[var(--text-tertiary)]
                   bg-[var(--bg-surface)]/40 backdrop-blur"
      >
        {chips.map((c) => (
          <button
            key={c.keys}
            type="button"
            onClick={c.onClick}
            disabled={!c.onClick}
            className="inline-flex items-center gap-1.5 hover:text-[var(--text-secondary)]
                       transition-colors duration-150 disabled:cursor-not-allowed"
          >
            <span className="font-mono text-[10px] text-[var(--text-secondary)]">
              {c.keys}
            </span>
            <span>{c.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
