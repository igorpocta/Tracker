/**
 * Inline SVG donut chart breaking down logged time by "project" — the
 * prefix of the issue key before the first hyphen (e.g. "ACME" from
 * "ACME-123").
 */
import { useMemo } from "react";

import type { WorklogRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";

export interface ProjectDonutChartProps {
  rows: WorklogRow[];
  /** Maximum number of slices to show; others bucket into "Other". */
  maxSlices?: number;
}

interface Slice {
  project: string;
  seconds: number;
  ratio: number;
  startAngle: number;
  endAngle: number;
  color: string;
}

const PALETTE = [
  "rgb(56 189 248)", // sky-400
  "rgb(34 197 94)", // green-500
  "rgb(168 85 247)", // purple-500
  "rgb(245 158 11)", // amber-500
  "rgb(239 68 68)", // red-500
  "rgb(20 184 166)", // teal-500
  "rgb(236 72 153)", // pink-500
  "rgb(99 102 241)", // indigo-500
];

export function ProjectDonutChart({
  rows,
  maxSlices = 8,
}: ProjectDonutChartProps) {
  const slices = useMemo<Slice[]>(() => {
    const totals = new Map<string, number>();
    for (const r of rows) {
      const prefix = (r.issue_key.split("-")[0] || "?").toUpperCase();
      totals.set(prefix, (totals.get(prefix) ?? 0) + r.duration_s);
    }
    const sorted = Array.from(totals.entries()).sort((a, b) => b[1] - a[1]);
    let trimmed: [string, number][];
    if (sorted.length > maxSlices) {
      const head = sorted.slice(0, maxSlices - 1);
      const rest = sorted.slice(maxSlices - 1).reduce((acc, [, s]) => acc + s, 0);
      trimmed = [...head, ["Other", rest]];
    } else {
      trimmed = sorted;
    }
    const total = trimmed.reduce((acc, [, s]) => acc + s, 0) || 1;
    let cursor = 0;
    return trimmed.map(([project, seconds], i) => {
      const ratio = seconds / total;
      const startAngle = cursor;
      const endAngle = cursor + ratio * Math.PI * 2;
      cursor = endAngle;
      return {
        project,
        seconds,
        ratio,
        startAngle,
        endAngle,
        color: i < PALETTE.length ? PALETTE[i] : "rgb(82 82 91)",
      };
    });
  }, [rows, maxSlices]);

  const total = slices.reduce((a, s) => a + s.seconds, 0);

  if (slices.length === 0 || total === 0) {
    return (
      <div className="text-xs text-neutral-500 py-6 text-center" data-testid="project-donut-empty">
        No project data in range.
      </div>
    );
  }

  const size = 160;
  const cx = size / 2;
  const cy = size / 2;
  const radius = 70;
  const inner = 42;

  return (
    <div className="flex items-center gap-4 flex-wrap" data-testid="project-donut-chart">
      <svg
        role="img"
        aria-label="Time logged by project"
        viewBox={`0 0 ${size} ${size}`}
        width={size}
        height={size}
        className="shrink-0"
      >
        {slices.map((s, i) => (
          <path
            key={i}
            d={donutSlicePath(cx, cy, radius, inner, s.startAngle, s.endAngle)}
            fill={s.color}
            opacity={0.92}
          >
            <title>{`${s.project} — ${formatDurationShort(s.seconds)}`}</title>
          </path>
        ))}
        <text
          x={cx}
          y={cy - 2}
          textAnchor="middle"
          fill="rgba(255,255,255,0.85)"
          fontSize={14}
          fontFamily="monospace"
        >
          {formatDurationShort(total)}
        </text>
        <text
          x={cx}
          y={cy + 12}
          textAnchor="middle"
          fill="rgba(255,255,255,0.4)"
          fontSize={9}
        >
          total
        </text>
      </svg>

      <ul className="flex-1 min-w-[180px] flex flex-col gap-1.5">
        {slices.map((s, i) => (
          <li key={i} className="flex items-center gap-2 text-xs">
            <span
              aria-hidden
              className="w-2.5 h-2.5 rounded-sm shrink-0"
              style={{ background: s.color }}
            />
            <span className="font-mono text-[11px] text-neutral-300 w-16 shrink-0">
              {s.project}
            </span>
            <span className="text-neutral-400 flex-1 text-right font-mono tabular-nums">
              {formatDurationShort(s.seconds)}
            </span>
            <span className="text-neutral-500 w-10 text-right text-[10px] font-mono">
              {Math.round(s.ratio * 100)}%
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * Build a donut-slice SVG path. Handles full-circle slices (rare but
 * possible when a single project consumes 100% of the range) by drawing
 * two half-circles.
 */
function donutSlicePath(
  cx: number,
  cy: number,
  outer: number,
  inner: number,
  startAngle: number,
  endAngle: number,
): string {
  const span = endAngle - startAngle;
  // For (nearly) full circles, fall back to two arcs to avoid SVG ambiguity.
  if (span >= Math.PI * 2 - 0.001) {
    return [
      `M ${cx + outer} ${cy}`,
      `A ${outer} ${outer} 0 1 1 ${cx - outer} ${cy}`,
      `A ${outer} ${outer} 0 1 1 ${cx + outer} ${cy}`,
      `Z`,
      `M ${cx + inner} ${cy}`,
      `A ${inner} ${inner} 0 1 0 ${cx - inner} ${cy}`,
      `A ${inner} ${inner} 0 1 0 ${cx + inner} ${cy}`,
      `Z`,
    ].join(" ");
  }

  const largeArc = span > Math.PI ? 1 : 0;
  // SVG angles: 0 at +X axis, sweeping clockwise — but we use math
  // convention (0 at +X, sweeping CCW). We flip by negating Y components.
  const outerStart = polar(cx, cy, outer, startAngle);
  const outerEnd = polar(cx, cy, outer, endAngle);
  const innerEnd = polar(cx, cy, inner, endAngle);
  const innerStart = polar(cx, cy, inner, startAngle);

  return [
    `M ${outerStart.x} ${outerStart.y}`,
    `A ${outer} ${outer} 0 ${largeArc} 1 ${outerEnd.x} ${outerEnd.y}`,
    `L ${innerEnd.x} ${innerEnd.y}`,
    `A ${inner} ${inner} 0 ${largeArc} 0 ${innerStart.x} ${innerStart.y}`,
    `Z`,
  ].join(" ");
}

function polar(cx: number, cy: number, r: number, theta: number) {
  return {
    x: cx + r * Math.cos(theta - Math.PI / 2),
    y: cy + r * Math.sin(theta - Math.PI / 2),
  };
}
