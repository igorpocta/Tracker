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

interface OpVisual {
  /** Czech label shown in the pill. */
  label: string;
  /** Token name for the background tint and text color. */
  toneVar: string;
  /** Lucide icon. */
  Icon: LucideIcon;
}

const OP_VISUALS: Record<string, OpVisual> = {
  create: { label: "Vytvořeno", toneVar: "--success", Icon: Plus },
  update: { label: "Změněno", toneVar: "--accent", Icon: Pencil },
  delete: { label: "Smazáno", toneVar: "--danger", Icon: Trash2 },
  move: { label: "Přesunuto", toneVar: "--accent-2", Icon: ArrowLeftRight },
  sync_tombstone: {
    label: "Smazáno mimo aplikaci",
    toneVar: "--text-tertiary",
    Icon: History,
  },
  restore: { label: "Obnoveno", toneVar: "--success", Icon: RotateCcw },
  revert: { label: "Vráceno", toneVar: "--warning", Icon: CornerUpLeft },
  retry: { label: "Opakováno", toneVar: "--accent", Icon: Check },
  undo: { label: "Vráceno mazání", toneVar: "--text-secondary", Icon: CornerUpLeft },
};

const FALLBACK: OpVisual = {
  label: "Akce",
  toneVar: "--text-tertiary",
  Icon: History,
};

export interface OpBadgeProps {
  op: AuditOp | string;
}

export function OpBadge({ op }: OpBadgeProps) {
  const v = OP_VISUALS[op] ?? FALLBACK;
  const tone = `var(${v.toneVar})`;
  const { Icon, label } = v;
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
