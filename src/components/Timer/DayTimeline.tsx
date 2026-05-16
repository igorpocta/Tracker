/**
 * Day overview timeline — Canvas-rendered.
 *
 * Renders today's worklogs as colored segments on a 06:00–22:00 axis. Místo
 * DOM elementů jeden `<canvas>` (Skia / CoreGraphics backed), aby drag a
 * hover nebyly limitovaný React render cyklem.
 *
 * Interakce:
 *   - hover → tooltip přes DOM (jeden absolutně pozicovaný div, nepotřebuje
 *     se rerender canvasu).
 *   - klik na segment → `onSelect(row)`.
 *   - **drag** uvnitř lokálního (nesyncovaného) segmentu → split. Při
 *     release pošle callback `onSplitRequest(row, splitAtMs)` který otevře
 *     dialog s pickerem druhého úkolu.
 *
 *   06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22
 *           [DEV-792==========][DEV-304========][DEV-926=]
 */
import { useCallback, useEffect, useRef, useState } from "react";

import type { WorklogRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";

/** First hour shown on the axis. */
const START_HOUR = 6;
/** Last hour shown (exclusive). */
const END_HOUR = 22;

const ROW_HEIGHT = 28;
const AXIS_HEIGHT = 16;
const SEGMENT_RADIUS = 3;
const MIN_DRAG_PX = 6;

export interface DayTimelineProps {
  rows: WorklogRow[];
  day: Date;
  onSelect?: (row: WorklogRow) => void;
  onSplitRequest?: (row: WorklogRow, splitAtMs: number) => void;
  /**
   * Tažením na prázdném místě uživatel definuje nový worklog. Callback
   * dostane unixovou hranici v ms (start + end zaokrouhlené dle taženého
   * intervalu) a parent route otevře dialog pro výběr úkolu.
   */
  onCreateRequest?: (startedAtMs: number, endedAtMs: number) => void;
}

interface Segment {
  row: WorklogRow;
  leftFrac: number;
  widthFrac: number;
}

interface Hover {
  segIdx: number;
  /** Canvas X v px (uvnitř <canvas>). */
  canvasX: number;
}

/** Drag uvnitř existujícího segmentu — split-request flow. */
interface SplitDragState {
  kind: "split";
  segIdx: number;
  startCanvasX: number;
  currentCanvasX: number;
}

/** Drag přes prázdné místo — create-request flow. */
interface CreateDragState {
  kind: "create";
  startCanvasX: number;
  currentCanvasX: number;
}

type DragState = SplitDragState | CreateDragState;

export function DayTimeline({
  rows,
  day,
  onSelect,
  onSplitRequest,
  onCreateRequest,
}: DayTimelineProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const segmentsRef = useRef<Segment[]>([]);
  const [hover, setHover] = useState<Hover | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);

  // Segmenty se přepočítají při změně rows / day.
  segmentsRef.current = buildSegments(rows, day);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const wrapper = wrapperRef.current;
    if (!canvas || !wrapper) return;
    const dpr = window.devicePixelRatio || 1;
    const cssWidth = wrapper.clientWidth;
    const cssHeight = AXIS_HEIGHT + ROW_HEIGHT;
    canvas.width = Math.round(cssWidth * dpr);
    canvas.height = Math.round(cssHeight * dpr);
    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${cssHeight}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssWidth, cssHeight);

    const totalHours = END_HOUR - START_HOUR;
    const hourW = cssWidth / totalHours;
    const accent = readCssVar("--accent") || "#14B8A6";
    const accentHover = readCssVar("--accent-hover") || accent;
    const muted = readCssVar("--text-tertiary") || "#71717a";
    const border = readCssVar("--border-subtle") || "rgba(0,0,0,0.1)";
    const bgApp = readCssVar("--bg-app") || "#ffffff";

    // Hour labels (osa). Render at the START of each hour bucket
    // (`6 7 8 …`) PLUS one trailing label at the right edge (`22`) so
    // segments ending late in the day don't look like they overflow
    // the last labelled hour. The trailing label is right-aligned so
    // it sits inside the canvas regardless of width.
    ctx.fillStyle = muted;
    ctx.font = "10px ui-monospace, SFMono-Regular, Menlo, monospace";
    ctx.textBaseline = "top";
    ctx.textAlign = "left";
    for (let i = 0; i < totalHours; i++) {
      ctx.fillText(String(START_HOUR + i), i * hourW + 2, 0);
    }
    ctx.textAlign = "right";
    ctx.fillText(String(END_HOUR), cssWidth - 2, 0);
    ctx.textAlign = "left";

    // Track background.
    const trackY = AXIS_HEIGHT;
    ctx.fillStyle = bgApp;
    roundRect(ctx, 0, trackY, cssWidth, ROW_HEIGHT, 4);
    ctx.fill();

    // Hour grid lines.
    ctx.strokeStyle = border;
    ctx.lineWidth = 1;
    for (let i = 1; i < totalHours; i++) {
      const x = Math.round(i * hourW) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, trackY);
      ctx.lineTo(x, trackY + ROW_HEIGHT);
      ctx.stroke();
    }

    // Segments.
    ctx.font = "600 10px ui-monospace, SFMono-Regular, Menlo, monospace";
    ctx.textBaseline = "middle";
    for (let idx = 0; idx < segmentsRef.current.length; idx++) {
      const seg = segmentsRef.current[idx];
      const x = (seg.leftFrac / totalHours) * cssWidth;
      const w = Math.max((seg.widthFrac / totalHours) * cssWidth, 1);
      const isHovered = hover?.segIdx === idx;
      ctx.fillStyle = isHovered ? accentHover : accent;
      roundRect(ctx, x, trackY + 2, w, ROW_HEIGHT - 4, SEGMENT_RADIUS);
      ctx.fill();

      // Label (issue key) — truncate by width.
      const label = seg.row.issue_key ?? "—";
      if (w > 24) {
        ctx.fillStyle = readCssVar("--accent-text") || "#ffffff";
        const trimmed = truncateToWidth(ctx, label, w - 8);
        ctx.fillText(trimmed, x + 4, trackY + ROW_HEIGHT / 2);
      }
    }

    // Drag overlay — vizuál závisí na kind:
    //   - split: tenká vertikální čára uvnitř existujícího segmentu.
    //   - create: poloprůhledný obdélník mezi start a current X.
    if (drag) {
      if (drag.kind === "split") {
        const seg = segmentsRef.current[drag.segIdx];
        if (seg) {
          const x = drag.currentCanvasX;
          const segLeftPx = (seg.leftFrac / totalHours) * cssWidth;
          const segRightPx =
            ((seg.leftFrac + seg.widthFrac) / totalHours) * cssWidth;
          const clampedX = Math.max(segLeftPx + 2, Math.min(segRightPx - 2, x));
          ctx.fillStyle = "rgba(0, 0, 0, 0.18)";
          ctx.fillRect(clampedX - 1, trackY, 2, ROW_HEIGHT);
        }
      } else {
        // create — protmavělý overlay v accent barvě s opacity.
        const x1 = Math.min(drag.startCanvasX, drag.currentCanvasX);
        const x2 = Math.max(drag.startCanvasX, drag.currentCanvasX);
        ctx.fillStyle = withAlpha(accent, 0.35);
        roundRect(ctx, x1, trackY + 2, Math.max(2, x2 - x1), ROW_HEIGHT - 4, SEGMENT_RADIUS);
        ctx.fill();
        // Tenké hranice levé/pravé (clearer visual feedback).
        ctx.fillStyle = accent;
        ctx.fillRect(x1 - 0.5, trackY, 1, ROW_HEIGHT);
        ctx.fillRect(x2 - 0.5, trackY, 1, ROW_HEIGHT);
      }
    }
  }, [drag, hover]);

  // Redraw při změně props i okenní velikosti.
  useEffect(() => {
    draw();
  }, [draw, rows, day]);
  useEffect(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;
    const obs = new ResizeObserver(() => draw());
    obs.observe(wrapper);
    return () => obs.disconnect();
  }, [draw]);

  const segAtPoint = useCallback((canvasX: number, canvasY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return -1;
    const cssWidth = canvas.clientWidth;
    const totalHours = END_HOUR - START_HOUR;
    if (canvasY < AXIS_HEIGHT || canvasY > AXIS_HEIGHT + ROW_HEIGHT) return -1;
    for (let i = 0; i < segmentsRef.current.length; i++) {
      const s = segmentsRef.current[i];
      const x = (s.leftFrac / totalHours) * cssWidth;
      const w = (s.widthFrac / totalHours) * cssWidth;
      if (canvasX >= x && canvasX <= x + w) return i;
    }
    return -1;
  }, []);

  return (
    <div
      ref={wrapperRef}
      className="rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] p-3"
      aria-label="Časová osa dne"
    >
      <h3 className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-2">
        Časová osa dne
      </h3>
      <div className="relative">
        <canvas
          ref={canvasRef}
          className="w-full block cursor-pointer"
          style={{ height: `${AXIS_HEIGHT + ROW_HEIGHT}px` }}
          onMouseMove={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            const idx = segAtPoint(x, y);
            if (drag) {
              setDrag({ ...drag, currentCanvasX: x });
            } else if (idx !== hover?.segIdx) {
              setHover(idx >= 0 ? { segIdx: idx, canvasX: x } : null);
            } else if (idx >= 0) {
              setHover({ segIdx: idx, canvasX: x });
            }
          }}
          onMouseLeave={() => {
            setHover(null);
            // Don't clear drag — uživatel může cursor vyjet ven a vrátit zpět.
          }}
          onMouseDown={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            const idx = segAtPoint(x, y);
            if (idx >= 0) {
              const seg = segmentsRef.current[idx];
              // Drag-to-split jen u lokálních (nesyncovaných) záznamů.
              if (seg.row.remote_id || seg.row.is_synced) return;
              setDrag({
                kind: "split",
                segIdx: idx,
                startCanvasX: x,
                currentCanvasX: x,
              });
            } else if (
              y >= AXIS_HEIGHT &&
              y <= AXIS_HEIGHT + ROW_HEIGHT &&
              onCreateRequest
            ) {
              // Prázdné místo v tracku → start create drag.
              setDrag({
                kind: "create",
                startCanvasX: x,
                currentCanvasX: x,
              });
            }
          }}
          onMouseUp={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const x = e.clientX - rect.left;
            if (drag) {
              const distance = Math.abs(x - drag.startCanvasX);
              if (drag.kind === "split") {
                const seg = segmentsRef.current[drag.segIdx];
                if (distance >= MIN_DRAG_PX && seg && onSplitRequest) {
                  const splitAtMs = canvasXToTimeMs(
                    x,
                    e.currentTarget.clientWidth,
                    day,
                  );
                  onSplitRequest(seg.row, splitAtMs);
                } else if (seg && onSelect) {
                  onSelect(seg.row);
                }
              } else if (drag.kind === "create" && onCreateRequest) {
                if (distance >= MIN_DRAG_PX) {
                  const cssWidth = e.currentTarget.clientWidth;
                  const startMs = canvasXToTimeMs(
                    Math.min(drag.startCanvasX, x),
                    cssWidth,
                    day,
                  );
                  const endMs = canvasXToTimeMs(
                    Math.max(drag.startCanvasX, x),
                    cssWidth,
                    day,
                  );
                  onCreateRequest(startMs, endMs);
                }
              }
              setDrag(null);
            } else {
              const idx = segAtPoint(x, e.clientY - rect.top);
              if (idx >= 0 && onSelect) {
                onSelect(segmentsRef.current[idx].row);
              }
            }
          }}
        />
        {hover && (
          <CanvasTooltip
            row={segmentsRef.current[hover.segIdx]?.row}
            x={hover.canvasX}
          />
        )}
      </div>
      <div className="mt-2 text-[10px] text-[var(--text-tertiary)]">
        Klikni pro zvýraznění. U lokálních záznamů můžeš tažením rozdělit
        blok na dva úkoly.
      </div>
    </div>
  );
}

function CanvasTooltip({
  row,
  x,
}: {
  row: WorklogRow | undefined;
  x: number;
}) {
  if (!row) return null;
  return (
    <div
      role="tooltip"
      className="absolute pointer-events-none z-20 px-2 py-1.5
                 rounded-[var(--radius-sm)] text-[11px] whitespace-nowrap"
      style={{
        left: `${x}px`,
        transform: "translate(-50%, calc(-100% - 6px))",
        background: "var(--bg-elevated)",
        color: "var(--text-primary)",
        border: "1px solid var(--border-default)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      <div className="font-medium">{row.issue_key ?? "(bez úkolu)"}</div>
      {row.summary && (
        <div className="text-[var(--text-tertiary)] max-w-[260px] truncate">
          {row.summary}
        </div>
      )}
      <div className="text-[var(--accent)] font-mono tabular-nums">
        {formatDurationShort(row.duration_s)}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Convert a CSS color string to one with alpha. Bere `#rrggbb`, `rgb(...)`,
 * nebo CSS variable hodnotu. Pokud nevíme jak parsovat, vrátí semi-průhledný
 * black jako fallback (lepší než crash).
 */
function withAlpha(color: string, alpha: number): string {
  const trimmed = color.trim();
  // #RRGGBB → rgba(r,g,b,a).
  if (trimmed.startsWith("#") && trimmed.length === 7) {
    const r = parseInt(trimmed.slice(1, 3), 16);
    const g = parseInt(trimmed.slice(3, 5), 16);
    const b = parseInt(trimmed.slice(5, 7), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
  if (trimmed.startsWith("rgb(") && trimmed.endsWith(")")) {
    return trimmed.replace("rgb(", "rgba(").replace(")", `, ${alpha})`);
  }
  // Unknown — fall through to a neutral overlay. (Tauri webview umí `color-mix`,
  // ale ne všechny prohlížeče.)
  return `rgba(0, 0, 0, ${alpha})`;
}

function readCssVar(name: string): string | undefined {
  if (typeof window === "undefined") return undefined;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name);
  return v ? v.trim() : undefined;
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const rad = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + rad, y);
  ctx.lineTo(x + w - rad, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + rad);
  ctx.lineTo(x + w, y + h - rad);
  ctx.quadraticCurveTo(x + w, y + h, x + w - rad, y + h);
  ctx.lineTo(x + rad, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - rad);
  ctx.lineTo(x, y + rad);
  ctx.quadraticCurveTo(x, y, x + rad, y);
  ctx.closePath();
}

function truncateToWidth(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxW: number,
): string {
  if (ctx.measureText(text).width <= maxW) return text;
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = Math.floor((lo + hi + 1) / 2);
    if (ctx.measureText(text.slice(0, mid) + "…").width <= maxW) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  return lo > 0 ? text.slice(0, lo) + "…" : "";
}

export function canvasXToTimeMs(
  canvasX: number,
  cssWidth: number,
  day: Date,
): number {
  const totalHours = END_HOUR - START_HOUR;
  const frac = Math.max(0, Math.min(1, canvasX / cssWidth));
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  // `frac * totalHours * 3_600_000` is float arithmetic; the result feeds
  // Tauri commands typed `i64` (`create_manual_worklog`, `split_worklog`).
  // Without the round, serde rejects with
  // `invalid type: floating point, expected i64`. Sub-ms precision is
  // meaningless for wall-clock worklogs anyway.
  return Math.round(
    dayStart.getTime() + (START_HOUR + frac * totalHours) * 3_600_000,
  );
}

export function buildSegments(rows: WorklogRow[], day: Date): Segment[] {
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  const windowStartMs = dayStart.getTime() + START_HOUR * 3_600_000;
  const windowEndMs = dayStart.getTime() + END_HOUR * 3_600_000;

  const out: Segment[] = [];
  for (const r of rows) {
    const a = r.started_at * 1000;
    const b = a + r.duration_s * 1000;
    const clampA = Math.max(a, windowStartMs);
    const clampB = Math.min(b, windowEndMs);
    if (clampB <= clampA) continue;
    const leftFrac = (clampA - windowStartMs) / 3_600_000;
    const widthFrac = (clampB - clampA) / 3_600_000;
    out.push({ row: r, leftFrac, widthFrac });
  }
  out.sort((x, y) => x.leftFrac - y.leftFrac);
  return out;
}

/**
 * Hour-fill computation pro starší testy (`bucketize` se v aplikaci už
 * aktivně nepoužívá — Canvas timeline má vlastní render path).
 */
export function bucketize(
  rows: WorklogRow[],
  day: Date,
): { hour: number; fill: number }[] {
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  const start = dayStart.getTime();
  const end = start + 86_400_000;

  const minutes = new Array<number>(END_HOUR - START_HOUR).fill(0);
  for (const r of rows) {
    const a = r.started_at * 1000;
    const b = a + r.duration_s * 1000;
    const clampA = Math.max(a, start);
    const clampB = Math.min(b, end);
    if (clampB <= clampA) continue;

    let cursor = clampA;
    while (cursor < clampB) {
      const d = new Date(cursor);
      const hour = d.getHours();
      const hourEnd = new Date(d);
      hourEnd.setMinutes(60, 0, 0);
      const slice = Math.min(clampB, hourEnd.getTime()) - cursor;
      if (hour >= START_HOUR && hour < END_HOUR) {
        minutes[hour - START_HOUR] = Math.min(
          60,
          minutes[hour - START_HOUR] + slice / 60_000,
        );
      }
      cursor += slice;
    }
  }
  return minutes.map((m, i) => ({ hour: START_HOUR + i, fill: m / 60 }));
}
