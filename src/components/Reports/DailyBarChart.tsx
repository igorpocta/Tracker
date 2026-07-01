/**
 * Daily-hours bar chart used on the Reports route.
 *
 * Sloupce jsou skládané (stacked) podle připojení — jeden segment na
 * Jira / Freelo / další providera (případně více). Lokálně vytvořené
 * záznamy bez `connection_id` se zobrazí jako neutrální segment „lokální".
 *
 * Phase 18B improvements:
 *   - Item 1: working-day shading přes `--accent-soft`.
 *   - Item 21: tooltip s rozpadem per provider, y-axis = max(observed, goal).
 */
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import { listConnections, listProjectColors } from "../../api/commands";
import { queryKeys } from "../../api/queryKeys";
import type { WorklogRow } from "../../api/types";
import { isWorkingDayLocal, useCalendarMask } from "../../hooks/useCalendarMask";
import { useT } from "../../i18n";
import { addDays, dayOverlapSeconds, dayStartUnixS, startOfDay } from "../../lib/dates";
import {
  formatDateCs,
  formatDateCsShort,
  formatDurationShort,
  formatWeekdayCs,
} from "../../lib/format";
import { usePrefsStore } from "../../stores/prefsStore";

export interface DailyBarChartProps {
  rows: WorklogRow[];
  from: Date;
  to: Date;
  /** Optional daily-goal anchor for the y-axis, in hours. */
  dailyGoalHours?: number;
}

interface Bucket {
  /** `null` = lokální záznam bez synchronizace. */
  connectionId: number | null;
  label: string;
  color: string;
  seconds: number;
}

interface LegendEntry {
  key: string;
  label: string;
  color: string;
}

const LOCAL_COLOR = "var(--text-tertiary)";

/**
 * Pokud připojení nemá explicit `config.color`, vezmeme výchozí accent
 * a každému dalšímu defaultnímu připojení snížíme alfa kanál:
 *   < 4 defaultů → krok 10 %
 *   ≥ 4 defaultů → krok 5 %  (víc connections = jemnější odstupňování)
 * Účel: vizuální odlišení připojení v stacked baru bez náhodných barev.
 */
function accentWithOpacity(percent: number): string {
  const p = Math.max(10, Math.min(100, Math.round(percent)));
  return `color-mix(in srgb, var(--accent) ${p}%, transparent)`;
}

export function DailyBarChart({ rows, from, to, dailyGoalHours }: DailyBarChartProps) {
  const t = useT();
  const days = useMemo(() => buildDayList(from, to), [from, to]);
  // V mono paletě je --accent-2 == --accent → goal line by se ztratila
  // ve sloupcích. Tehdy zůstáváme u zlaté `--warning`. Dual paleta dovolí
  // line dotáhnout na sekundární accent (zlatá kolize zmizí).
  const paletteMode = usePrefsStore((s) => s.paletteMode);
  const goalLineColor =
    paletteMode === "dual" ? "var(--accent-2)" : "var(--warning, #ff9f0a)";

  // Připojení potřebujeme pro pojmenování / barvy segmentů. Selže-li
  // (např. během startup), padá fallback na "lokální" pro vše.
  const connectionsQ = useQuery({
    queryKey: queryKeys.connections.all(),
    queryFn: listConnections,
    staleTime: 60_000,
  });

  // Volitelné per-project barevné overridy. Když je seznam neprázdný,
  // worklog s odpovídajícím prefixem klíče přebere barvu odsud místo
  // defaultní per-connection palety.
  const projectColorsQ = useQuery({
    queryKey: ["project-colors"],
    queryFn: listProjectColors,
    staleTime: 60_000,
  });

  const connInfo = useMemo(
    () => buildConnectionMap(connectionsQ.data ?? []),
    [connectionsQ.data],
  );

  const projectColorMap = useMemo(() => {
    const m = new Map<string, string>();
    for (const p of projectColorsQ.data ?? []) {
      m.set(p.project_key, p.color);
    }
    return m;
  }, [projectColorsQ.data]);

  // Per-day mapování connection_id (nebo null) → seconds. Když existuje
  // project-color override, řadíme worklog do vlastního bucketu, ne do
  // connection bucketu — tak vidíš v reportu projektovou distribuci, ne
  // jen jednu globální Jira/Freelo barvu.
  const bucketsByDay = useMemo(
    () => groupByDay(rows, connInfo, projectColorMap),
    [rows, connInfo, projectColorMap],
  );

  // Phase 18B — Item 1: per-day working-day state.
  const { mask, nonWorking } = useCalendarMask(from, to);

  const observedMaxHours = useMemo(() => {
    let m = 0;
    for (const d of days) {
      const total = sumBuckets(bucketsByDay.get(formatKey(d)));
      const v = total / 3600;
      if (v > m) m = v;
    }
    return m;
  }, [days, bucketsByDay]);

  const max = useMemo(() => {
    const base = Math.max(observedMaxHours, dailyGoalHours ?? 0, 3);
    return Math.max(3, Math.ceil(base / 3) * 3);
  }, [observedMaxHours, dailyGoalHours]);

  const [hover, setHover] = useState<number | null>(null);

  const goalLinePct =
    dailyGoalHours && dailyGoalHours > 0 && max > 0
      ? Math.min(100, (dailyGoalHours / max) * 100)
      : null;

  // Legenda = sjednocení všech bucketů, které mají alespoň jednu vteřinu.
  const legend = useMemo(() => buildLegend(bucketsByDay), [bucketsByDay]);

  // Pro delší období řídneme x-axis popisky, ať se nepřekrývají:
  //   ≤ 14 dní  → každý den
  //   ≤ 31 dní  → každý druhý den (původní chování)
  //   ≤ 90 dní  → po týdnu, zarovnáno na pondělky
  //   > 90 dní  → po dvou týdnech, zarovnáno na pondělky
  // Když stepneme po týdnech, posuneme anchor na první pondělí v rozsahu,
  // aby labely vizuálně padaly na začátky týdnů (uživatel snáz čte).
  const { labelStep, labelAnchor } = useMemo(() => {
    const n = days.length;
    const step = n <= 14 ? 1 : n <= 31 ? 2 : n <= 90 ? 7 : 14;
    if (step < 7) return { labelStep: step, labelAnchor: 0 };
    const firstMonday = days.findIndex((d) => d.getDay() === 1);
    return { labelStep: step, labelAnchor: firstMonday >= 0 ? firstMonday : 0 };
  }, [days]);

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <div className="flex items-center justify-between mb-3 gap-4 flex-wrap">
        <h3 className="text-sm font-semibold text-[var(--text-primary)]">
          {t("reports.chart.heading")}
        </h3>
        {legend.length > 0 && (
          <div className="flex items-center gap-3 text-[11px] text-[var(--text-secondary)]">
            {legend.map((l) => (
              <div key={l.key} className="flex items-center gap-1.5">
                <span
                  className="inline-block w-2.5 h-2.5 rounded-sm"
                  style={{ background: l.color }}
                  aria-hidden
                />
                <span>{l.label}</span>
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="flex gap-4 h-[260px]">
        <div className="flex flex-col justify-between text-[10px] text-[var(--text-tertiary)] tabular-nums py-1">
          {[max, Math.round((max * 2) / 3), Math.round(max / 3), 0].map((v) => (
            <div key={`y-${v}`}>{v}</div>
          ))}
        </div>
        <div className="flex-1 relative">
          <div className="absolute inset-0 pt-1 pb-5">
            <div className="relative h-full w-full">
              {/* Grid lines */}
              <div className="absolute inset-0 flex flex-col justify-between pointer-events-none">
                {[0, 1, 2, 3].map((i) => (
                  <div
                    key={`grid-${i}`}
                    className="border-t border-[var(--border-subtle)]"
                  />
                ))}
              </div>

              {/* Working-day shading bands (sit behind the bars) */}
              <div className="absolute inset-0 flex gap-[2px] pointer-events-none">
                {days.map((d) => {
                  const working = isWorkingDayLocal(d, mask, nonWorking);
                  return (
                    <div
                      key={`band-${d.toISOString()}`}
                      className="flex-1 rounded-[2px]"
                      style={{
                        background: working
                          ? "var(--accent-soft)"
                          : "transparent",
                        opacity: working ? 0.35 : 0,
                      }}
                    />
                  );
                })}
              </div>

              {/* Stacked bars */}
              <div className="absolute inset-0 flex items-end gap-[2px]">
                {days.map((d, idx) => {
                  const buckets = bucketsByDay.get(formatKey(d)) ?? [];
                  const total = sumBuckets(buckets);
                  const totalPct = max > 0 ? (total / 3600 / max) * 100 : 0;
                  const isHovered = hover === idx;
                  return (
                    <div
                      key={d.toISOString()}
                      className="flex-1 h-full relative cursor-pointer"
                      onMouseEnter={() => setHover(idx)}
                      onMouseLeave={() => setHover(null)}
                    >
                      {totalPct > 0 && (
                        <div
                          className="absolute left-0 right-0 bottom-0 flex flex-col-reverse rounded-t-[2px] overflow-hidden transition-opacity duration-150"
                          style={{
                            height: `${totalPct}%`,
                            minHeight: "3px",
                            opacity: isHovered ? 0.85 : 1,
                          }}
                        >
                          {buckets
                            .filter((b) => b.seconds > 0)
                            .map((b, segIdx, arr) => {
                              const segPct = (b.seconds / total) * 100;
                              return (
                                <div
                                  key={`${b.connectionId ?? "local"}`}
                                  style={{
                                    height: `${segPct}%`,
                                    background: b.color,
                                    // Vrchní segment dostane mírnou tečku
                                    // mezi sebou a dalším, ať se barvy
                                    // vizuálně neslévají do jedné plochy.
                                    borderTop:
                                      segIdx < arr.length - 1
                                        ? "1px solid rgba(255,255,255,0.15)"
                                        : "none",
                                  }}
                                />
                              );
                            })}
                        </div>
                      )}
                      {isHovered && total > 0 && (
                        <DailyTooltip
                          date={d}
                          buckets={buckets.filter((b) => b.seconds > 0)}
                          total={total}
                        />
                      )}
                    </div>
                  );
                })}
              </div>

              {/* Daily-goal line */}
              {goalLinePct !== null && (
                <div
                  className="absolute left-0 right-0 pointer-events-none"
                  style={{ bottom: `${goalLinePct}%` }}
                  aria-label={t("reports.chart.goalAria", {
                    hours: dailyGoalHours ?? 0,
                  })}
                >
                  <div
                    className="w-full"
                    style={{
                      borderTop: `1px dashed ${goalLineColor}`,
                      opacity: 0.8,
                    }}
                  />
                  <div
                    className="absolute -top-2 right-0 px-1 py-[1px] rounded-[3px] text-[9px] font-medium tabular-nums leading-none"
                    style={{
                      background: "var(--bg-surface)",
                      color: goalLineColor,
                      border: `1px solid ${goalLineColor}`,
                      opacity: 0.85,
                    }}
                  >
                    {t("reports.chart.goalLabel", { hours: dailyGoalHours ?? 0 })}
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* X-axis labels.
              `whitespace-nowrap` + `overflow-visible` umožní vykreslit `D. M.`
              v jednom řádku, i když flex-1 buňka je užší než text — sousední
              prázdné buňky pojmou přesah, takže se popisky nezalamují.   */}
          <div className="absolute left-0 right-0 bottom-0 flex gap-[2px] overflow-visible">
            {days.map((d, idx) => {
              const showLabel =
                idx >= labelAnchor && (idx - labelAnchor) % labelStep === 0;
              return (
                <div
                  key={`label-${d.toISOString()}`}
                  className="flex-1 text-[9px] text-[var(--text-tertiary)] text-center tabular-nums whitespace-nowrap"
                >
                  {showLabel ? formatDateCsShort(d) : ""}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

function DailyTooltip({
  date,
  buckets,
  total,
}: {
  date: Date;
  buckets: Bucket[];
  total: number;
}) {
  const t = useT();
  return (
    <div
      role="tooltip"
      className="absolute left-1/2 -translate-x-1/2 bottom-[calc(100%+4px)] z-20
                 px-2 py-1.5 rounded-[var(--radius-sm)] text-[11px] whitespace-nowrap
                 pointer-events-none"
      style={{
        background: "var(--bg-elevated)",
        color: "var(--text-primary)",
        border: "1px solid var(--border-default)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      <div className="font-medium">
        {formatWeekdayCs(date)} · {formatDateCs(date)}
      </div>
      <div className="text-[var(--text-tertiary)] mb-1">
        {t("reports.chart.tooltipTotal", {
          duration: formatDurationShort(total),
        })}
      </div>
      <div className="flex flex-col gap-0.5">
        {buckets.map((b) => (
          <div
            key={b.connectionId ?? "local"}
            className="flex items-center gap-1.5"
          >
            <span
              className="inline-block w-2 h-2 rounded-sm"
              style={{ background: b.color }}
              aria-hidden
            />
            <span className="flex-1">{b.label}</span>
            <span className="font-mono tabular-nums ml-2">
              {formatDurationShort(b.seconds)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// -----------------------------------------------------------------------------
// Pure helpers (kept exported where the test file references them)
// -----------------------------------------------------------------------------

interface ConnInfo {
  label: string;
  color: string;
}

export function buildConnectionMap(
  list: {
    id: number;
    provider: string;
    name: string;
    config?: Record<string, unknown>;
  }[],
): Map<number, ConnInfo> {
  const out = new Map<number, ConnInfo>();
  // Spočítáme, kolik připojení nemá vlastní barvu — určuje krok průhlednosti.
  const defaultsCount = list.filter(
    (c) => !(typeof c.config?.["color"] === "string" && c.config["color"]),
  ).length;
  const stepPct = defaultsCount < 4 ? 10 : 5;

  let defaultIdx = 0;
  for (const c of list) {
    const raw = c.config?.["color"];
    const customColor = typeof raw === "string" && raw ? raw : null;
    let color: string;
    if (customColor) {
      color = customColor;
    } else {
      color = accentWithOpacity(100 - defaultIdx * stepPct);
      defaultIdx++;
    }
    out.set(c.id, { label: c.name || c.provider, color });
  }
  return out;
}

function groupByDay(
  rows: WorklogRow[],
  conn: Map<number, ConnInfo>,
  projectColors: Map<string, string>,
): Map<string, Bucket[]> {
  const days = new Map<string, Map<string, Bucket>>();
  for (const r of rows) {
    // Hierarchie barev: 1) explicit project color, 2) connection color,
    // 3) "lokální" placeholder. Bucket key sloučí worklogy se stejnou
    // barvou+labelem, ať se nezobrazí 5 stejně barevných pruhů vedle sebe.
    const projectKey = projectKeyFromIssue(r.issue_key);
    const projectOverride = projectKey ? projectColors.get(projectKey) : undefined;

    let info: { label: string; color: string };
    let bucketKey: string;
    if (projectOverride) {
      info = { label: projectKey!, color: projectOverride };
      bucketKey = `p:${projectKey}`;
    } else if (r.connection_id != null) {
      info = conn.get(r.connection_id) ?? {
        label: `#${r.connection_id}`,
        color: accentWithOpacity(60),
      };
      bucketKey = `c:${r.connection_id}`;
    } else {
      info = { label: "Lokální", color: LOCAL_COLOR };
      bucketKey = "local";
    }

    // Variant B (feedback #2): split a worklog across every local day it
    // touches, crediting each day only its overlapping slice — a 23:30→00:30
    // entry adds 30 min to each day, not 60 min to the start day.
    const endedAt = r.ended_at ?? r.started_at + r.duration_s;
    let dayDate = startOfDay(new Date(r.started_at * 1000));
    const lastDay = startOfDay(new Date(endedAt * 1000));
    while (dayDate.getTime() <= lastDay.getTime()) {
      const dayStart = dayStartUnixS(dayDate);
      const dayEnd = dayStartUnixS(addDays(dayDate, 1));
      const secs = dayOverlapSeconds(r.started_at, endedAt, dayStart, dayEnd);
      if (secs > 0) {
        const k = formatKey(dayDate);
        let dayMap = days.get(k);
        if (!dayMap) {
          dayMap = new Map();
          days.set(k, dayMap);
        }
        const existing = dayMap.get(bucketKey);
        if (existing) {
          existing.seconds += secs;
        } else {
          dayMap.set(bucketKey, {
            connectionId: r.connection_id ?? null,
            label: info.label,
            color: info.color,
            seconds: secs,
          });
        }
      }
      dayDate = addDays(dayDate, 1);
    }
  }

  // Stabilní pořadí: nejdřív řadit dle labelu (project_key / connection name),
  // lokální (`connectionId === null` a label "Lokální") na konec.
  const out = new Map<string, Bucket[]>();
  for (const [k, map] of days) {
    const arr = Array.from(map.values()).sort((a, b) => {
      const aLocal = a.label === "Lokální";
      const bLocal = b.label === "Lokální";
      if (aLocal && !bLocal) return 1;
      if (!aLocal && bLocal) return -1;
      return a.label.localeCompare(b.label, "cs", { sensitivity: "base" });
    });
    out.set(k, arr);
  }
  return out;
}

/**
 * Z issue klíče odvodí "project key" — prefix pro Jira (`DEV-792` → `DEV`),
 * konkrétní task pro Freelo (`FREELO-12345`). Pokud uživatel chce barvu
 * pro celý Freelo projekt, musí ji nastavit na konkrétní `FREELO-P-…`
 * (parent_key); ten však tady bez extra lookupu nevíme, takže Freelo
 * worklog zatím dostane fallback per-connection.
 */
function projectKeyFromIssue(issueKey?: string | null): string | null {
  if (!issueKey) return null;
  if (issueKey.startsWith("FREELO-P-")) return issueKey;
  if (issueKey.startsWith("FREELO-")) return issueKey;
  const dash = issueKey.indexOf("-");
  return dash > 0 ? issueKey.slice(0, dash) : issueKey;
}

function sumBuckets(buckets: Bucket[] | undefined): number {
  if (!buckets) return 0;
  let s = 0;
  for (const b of buckets) s += b.seconds;
  return s;
}

function buildLegend(buckets: Map<string, Bucket[]>): LegendEntry[] {
  const seen = new Map<string, LegendEntry>();
  for (const arr of buckets.values()) {
    for (const b of arr) {
      if (b.seconds <= 0) continue;
      const key = b.connectionId === null ? "local" : `c:${b.connectionId}`;
      if (!seen.has(key)) {
        seen.set(key, { key, label: b.label, color: b.color });
      }
    }
  }
  // Lokální vždy poslední.
  return Array.from(seen.values()).sort((a, b) => {
    if (a.key === "local") return 1;
    if (b.key === "local") return -1;
    return a.label.localeCompare(b.label);
  });
}

function buildDayList(from: Date, to: Date): Date[] {
  const out: Date[] = [];
  const d = startOfDay(from);
  const end = startOfDay(to);
  while (d <= end) {
    out.push(new Date(d));
    d.setDate(d.getDate() + 1);
  }
  return out;
}

export function formatKey(d: Date): string {
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

// Re-export pro testy.
export { addDays, useQuery };
