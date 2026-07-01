/**
 * Colored pill identifying the kind of audit operation.
 *
 * Uses only token-based colors (no hard-coded hex). For ops outside our
 * built-in set the badge falls through to a neutral "info" tint with the
 * raw op string — useful if a future migration introduces a new kind that
 * the UI hasn't been taught about yet.
 */
import {
  ArrowLeftRight,
  Check,
  CornerUpLeft,
  History,
  Pencil,
  Plus,
  RotateCcw,
  Trash2,
  type LucideIcon,
} from "lucide-react";

import type { AuditOp } from "../../api/types";
import { useT } from "../../i18n";

interface OpVisual {
  /** i18n key for the label shown in the pill. */
  labelKey: string;
  /** Token name for the background tint and text color. */
  toneVar: string;
  /** Lucide icon. */
  Icon: LucideIcon;
}

const OP_VISUALS: Record<string, OpVisual> = {
  create: { labelKey: "audit.op.create", toneVar: "--success", Icon: Plus },
  update: { labelKey: "audit.op.update", toneVar: "--accent", Icon: Pencil },
  delete: { labelKey: "audit.op.delete", toneVar: "--danger", Icon: Trash2 },
  move: { labelKey: "audit.op.move", toneVar: "--accent-2", Icon: ArrowLeftRight },
  sync_tombstone: {
    labelKey: "audit.op.syncTombstone",
    toneVar: "--text-tertiary",
    Icon: History,
  },
  restore: { labelKey: "audit.op.restore", toneVar: "--success", Icon: RotateCcw },
  revert: { labelKey: "audit.op.revert", toneVar: "--warning", Icon: CornerUpLeft },
  retry: { labelKey: "audit.op.retry", toneVar: "--accent", Icon: Check },
  undo: { labelKey: "audit.op.undo", toneVar: "--text-secondary", Icon: CornerUpLeft },
};

const FALLBACK: OpVisual = {
  labelKey: "audit.op.fallback",
  toneVar: "--text-tertiary",
  Icon: History,
};

export interface OpBadgeProps {
  op: AuditOp | string;
}

export function OpBadge({ op }: OpBadgeProps) {
  const t = useT();
  const v = OP_VISUALS[op] ?? FALLBACK;
  const tone = `var(${v.toneVar})`;
  const { Icon, labelKey } = v;
  const label = t(labelKey);
  return (
    <span
      className="inline-flex items-center gap-1 h-5 px-1.5 rounded-[var(--radius-sm)]
                 text-[10px] font-medium uppercase tracking-[0.06em]"
      style={{
        background: `color-mix(in srgb, ${tone} 14%, transparent)`,
        color: tone,
        border: `1px solid color-mix(in srgb, ${tone} 28%, transparent)`,
      }}
    >
      <Icon className="w-3 h-3" aria-hidden />
      {label}
    </span>
  );
}
