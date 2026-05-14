/**
 * Inline SVG donut chart breaking down logged time by "project" — the
 * prefix of the issue key before the first hyphen (e.g. "ACME" from
 * "ACME-123").
 *
 * Slices use analogous tints of the accent color: lightness varies from
 * 60% to 35% so consecutive segments stay distinct without introducing
 * unrelated hues. The result reads as a single graphic, not a rainbow.
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
  /** Lightness % to apply to the accent hue. */
  lightness: number;
}

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
    const n = trimmed.length;
    return trimmed.map(([project, seconds], i) => {
      const ratio = seconds / total;
      const startAngle = cursor;
      const endAngle = cursor + ratio * Math.PI * 2;
      cursor = endAngle;
      // Lightness curve: biggest slice 60%, smallest ~35%.
      const t = n <= 1 ? 0 : i / (n - 1);
      const lightness = 60 - t * 25;
      return {
        project,
        seconds,
        ratio,
        startAngle,
        endAngle,
        lightness,
      };
    });
  }, [rows, maxSlices]);

  const total = slices.reduce((a, s) => a + s.seconds, 0);

  if (slices.length === 0 || total === 0) {
    return (
      <div
        className="text-xs text-[var(--text-tertiary)] py-6 text-center"
        data-testid="project-donut-empty"
      >
        No project data in range.
      </div>
    );
  }

  const size = 160;
  const cx = size / 2;
  const cy = size / 2;
  const radius = 70;
  const inner = 44;

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
            fill={`hsl(var(--accent-h) var(--accent-s) ${s.lightness}%)`}
          >
            <title>{`${s.project} — ${formatDurationShort(s.seconds)}`}</title>
          </path>
        ))}
        <text
          x={cx}
          y={cy - 2}
          textAnchor="middle"
          fill="var(--text-primary)"
          fontSize={14}
          fontFamily="monospace"
        >
          {formatDurationShort(total)}
        </text>
        <text
          x={cx}
          y={cy + 12}
          textAnchor="middle"
          fill="var(--text-tertiary)"
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
              className="w-2 h-2 rounded-sm shrink-0"
              style={{
                background: `hsl(var(--accent-h) var(--accent-s) ${s.lightness}%)`,
              }}
            />
            <span className="font-mono text-[11px] text-[var(--text-secondary)] w-16 shrink-0 uppercase">
              {s.project}
            </span>
            <span className="text-[var(--text-tertiary)] flex-1 text-right font-mono tabular-nums">
              {formatDurationShort(s.seconds)}
            </span>
            <span className="text-[var(--text-tertiary)] w-10 text-right text-[10px] font-mono">
              {Math.round(s.ratio * 100)}%
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function donutSlicePath(
  cx: number,
  cy: number,
  outer: number,
  inner: number,
  startAngle: number,
  endAngle: number,
): string {
  const span = endAngle - startAngle;
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
