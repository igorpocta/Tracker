/**
 * Right-click context menu shown on Calendar day cells.
 *
 * Phase 18C — quick non-working-day toggling without leaving the grid.
 *
 *   ┌─────────────────────────────────┐
 *   │  Označit jako nepracovní den ▸  │   ← expands the reason picker
 *   │  Detail dne                     │
 *   └─────────────────────────────────┘
 *
 * When the user picks "Označit jako nepracovní den", the same panel swaps
 * to the reason chooser (Dovolená / Svátek / Osobní) and the parent fires
 * `addNonWorkingDay`. If the day is already marked, the first item is the
 * unmark action instead.
 *
 * The menu positions itself at the click coordinates and closes on Escape,
 * outside-click or any selection. It is rendered as a `position: fixed`
 * floating panel so it stays where the cursor was even when the grid scrolls.
 */
import { useEffect, useRef, useState } from "react";

import { useT } from "../../i18n";

export type NonWorkingReason = "vacation" | "holiday" | "personal";

export interface CellContextMenuProps {
  /** Pixel coordinates of the click (viewport-relative). */
  x: number;
  y: number;
  /** ISO date `YYYY-MM-DD` the menu was opened on. Display only. */
  date: string;
  /**
   * True when the day is currently working (mask says yes AND it is not in
   * the non-working set). Drives the wording of the first item: "Mark as
   * non-working" vs "Mark as working".
   */
  isWorkingDay: boolean;
  /** True if there is an explicit `non_working_days` row for this date. */
  isExplicitlyMarked: boolean;
  /** Fired when the user picks a reason from the sub-menu. */
  onMarkNonWorking: (reason: NonWorkingReason) => void;
  /** Fired when the user picks "Mark as working" — removes the row. */
  onUnmark: () => void;
  /** Open the existing right-pane day detail. */
  onOpenDetail: () => void;
  /** Dismiss the menu (Escape, outside-click, or any selection). */
  onClose: () => void;
}

const REASONS: { value: NonWorkingReason; icon: string; labelKey: string }[] = [
  { value: "vacation", icon: "🏖", labelKey: "misc.cellMenu.reason.vacation" },
  { value: "holiday", icon: "🎉", labelKey: "misc.cellMenu.reason.holiday" },
  { value: "personal", icon: "🙅", labelKey: "misc.cellMenu.reason.personal" },
];

export function CellContextMenu(props: CellContextMenuProps) {
  const {
    x,
    y,
    isWorkingDay,
    isExplicitlyMarked,
    onMarkNonWorking,
    onUnmark,
    onOpenDetail,
    onClose,
  } = props;

  const t = useT();
  const ref = useRef<HTMLDivElement | null>(null);
  /** When true the panel shows the reason sub-menu instead of the root items. */
  const [showReasonPicker, setShowReasonPicker] = useState(false);

  // Outside-click + Escape handling. Pointerdown beats click so we close
  // before the underlying cell handles its own click (which would otherwise
  // open the day detail unintentionally).
  useEffect(() => {
    function handleDown(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    document.addEventListener("pointerdown", handleDown);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("pointerdown", handleDown);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  // Clamp into the viewport so a click near the right/bottom edge doesn't
  // push the menu off-screen.
  const MENU_W = 240;
  const MENU_H = 160;
  const left = Math.min(x, window.innerWidth - MENU_W - 8);
  const top = Math.min(y, window.innerHeight - MENU_H - 8);

  return (
    <div
      ref={ref}
      role="menu"
      aria-label={t("misc.cellMenu.aria")}
      data-testid="cell-context-menu"
      className="fixed z-50 min-w-[220px] py-1 rounded-[var(--radius-md)]"
      style={{
        left,
        top,
        background: "var(--bg-elevated)",
        border: "1px solid var(--border-default)",
        boxShadow: "var(--shadow-md, 0 8px 24px rgba(0,0,0,0.35))",
        color: "var(--text-primary)",
      }}
    >
      {!showReasonPicker && (
        <>
          {isExplicitlyMarked || !isWorkingDay ? (
            <MenuItem
              onClick={() => {
                onUnmark();
                onClose();
              }}
            >
              {t("misc.cellMenu.markWorking")}
            </MenuItem>
          ) : (
            <MenuItem
              onClick={() => setShowReasonPicker(true)}
              hasChevron
            >
              {t("misc.cellMenu.markNonWorking")}
            </MenuItem>
          )}
          <Separator />
          <MenuItem
            onClick={() => {
              onOpenDetail();
              onClose();
            }}
          >
            {t("misc.cellMenu.dayDetail")}
          </MenuItem>
        </>
      )}

      {showReasonPicker && (
        <>
          <div
            className="px-3 py-1 text-[10px] uppercase tracking-[0.12em]"
            style={{ color: "var(--text-tertiary)" }}
          >
            {t("misc.cellMenu.reason")}
          </div>
          {REASONS.map((r) => (
            <MenuItem
              key={r.value}
              onClick={() => {
                onMarkNonWorking(r.value);
                onClose();
              }}
            >
              <span className="mr-2" aria-hidden>
                {r.icon}
              </span>
              {t(r.labelKey)}
            </MenuItem>
          ))}
          <Separator />
          <MenuItem onClick={() => setShowReasonPicker(false)}>
            {t("misc.cellMenu.back")}
          </MenuItem>
        </>
      )}
    </div>
  );
}

function MenuItem({
  children,
  onClick,
  hasChevron,
}: {
  children: React.ReactNode;
  onClick: () => void;
  hasChevron?: boolean;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className="w-full flex items-center justify-between text-left px-3 h-8 text-sm
                 transition-colors duration-150
                 hover:bg-[var(--bg-hover)] focus:bg-[var(--bg-hover)] outline-none"
    >
      <span className="flex items-center">{children}</span>
      {hasChevron && (
        <span aria-hidden style={{ color: "var(--text-tertiary)" }}>
          ›
        </span>
      )}
    </button>
  );
}

function Separator() {
  return (
    <div
      role="separator"
      className="my-1 h-px"
      style={{ background: "var(--border-subtle)" }}
    />
  );
}
