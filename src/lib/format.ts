/**
 * Small set of duration / time formatters shared across the main app.
 *
 * All helpers are pure functions over numbers; they have no React or Tauri
 * dependency so they're trivial to unit-test from Vitest.
 */

/** Pad a non-negative integer to at least 2 digits with leading zeros. */
function pad2(n: number): string {
  return n < 10 ? `0${n}` : `${n}`;
}

/**
 * Format a duration (in **seconds**) as `HH:MM:SS`. Negative values are
 * clamped to 0. The hours component is not truncated to 2 digits — a
 * marathon timer reading `123:45:06` is unusual but legitimate.
 */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "00:00:00";
  const s = Math.floor(seconds);
  const hours = Math.floor(s / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  const secs = s % 60;
  return `${pad2(hours)}:${pad2(minutes)}:${pad2(secs)}`;
}

/**
 * Short human-readable duration: `2h 15m`, `15m`, `45s`. Useful for the
 * worklog history rows where `HH:MM:SS` precision would be noisy.
 */
export function formatDurationShort(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "0m";
  const s = Math.floor(seconds);
  const hours = Math.floor(s / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }
  if (minutes > 0) return `${minutes}m`;
  return `${s}s`;
}

/** Format hours like `7.5h` with one decimal, trimming trailing zero. */
export function formatHours(hours: number): string {
  if (!Number.isFinite(hours) || hours <= 0) return "0h";
  const rounded = Math.round(hours * 10) / 10;
  // Strip trailing `.0`.
  const s = Number.isInteger(rounded) ? `${rounded}` : `${rounded.toFixed(1)}`;
  return `${s}h`;
}

/**
 * Czech relative time-ago. Operates on either a Date or a unix-epoch number
 * (we try to detect seconds vs. milliseconds automatically: anything < 1e12 is
 * treated as seconds). Always rounds to the nearest unit.
 */
export function formatRelativeTime(
  value: Date | number,
  now: Date = new Date(),
): string {
  const past =
    value instanceof Date
      ? value
      : new Date(value < 1e12 ? value * 1000 : value);
  const diffMs = now.getTime() - past.getTime();
  if (diffMs < 0) return "právě teď";
  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 45) return "právě teď";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `před ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `před ${hours} h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `před ${days} dny`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `před ${weeks} týd`;
  const months = Math.floor(days / 30);
  if (months < 12) return `před ${months} měs`;
  const years = Math.floor(days / 365);
  return `před ${years} r`;
}

/**
 * Czech-locale short date: `14. 5. 2026`. Stable output: never falls back to
 * a different locale (the previous `dd/mm/yyyy` shape is gone).
 */
export function formatDateCs(d: Date): string {
  return `${d.getDate()}. ${d.getMonth() + 1}. ${d.getFullYear()}`;
}

/** Czech-locale month + day without year: `14. 5.` */
export function formatDateCsShort(d: Date): string {
  return `${d.getDate()}. ${d.getMonth() + 1}.`;
}

/** Format a timestamp as a `HH:MM` clock time in the user's local timezone. */
export function formatClockTime(value: Date | number): string {
  const d =
    value instanceof Date
      ? value
      : new Date(value < 1e12 ? value * 1000 : value);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

/**
 * Returns true iff `valueSeconds` (unix epoch in seconds) falls within
 * today's calendar day in the supplied (or system) timezone.
 */
export function isToday(valueSeconds: number, now: Date = new Date()): boolean {
  const d = new Date(valueSeconds * 1000);
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  );
}

/**
 * Format a money amount in the user's currency.
 *
 * Conventions:
 * - CZK / PLN: suffix with a thin space (`1 234 Kč`, `1 234 zł`).
 * - EUR / USD / GBP / CHF: prefix with the symbol, 2 decimals.
 *
 * Falls back to ISO code suffix for anything unrecognized.
 */
export function formatMoney(amount: number, currency: string): string {
  if (!Number.isFinite(amount)) return "—";
  const code = (currency || "CZK").toUpperCase();
  const thinSpace = " ";

  // CZK / PLN — suffix with native sign, rounded to whole units (CZK is
  // typically not displayed with hellers in modern apps).
  if (code === "CZK") {
    const rounded = Math.round(amount);
    return `${groupThinSpace(rounded)}${thinSpace}Kč`;
  }
  if (code === "PLN") {
    const rounded = Math.round(amount * 100) / 100;
    return `${groupThinSpace(rounded)}${thinSpace}zł`;
  }

  // Symbol-prefixed currencies with 2 decimals.
  const prefixMap: Record<string, string> = {
    EUR: "€",
    USD: "$",
    GBP: "£",
    CHF: "CHF ",
  };
  const prefix = prefixMap[code];
  const rounded = Math.round(amount * 100) / 100;
  if (prefix) {
    return `${prefix}${groupComma(rounded)}`;
  }
  return `${groupComma(rounded)}${thinSpace}${code}`;
}

/** Number grouping with thin-space (1 234 567,89). */
function groupThinSpace(n: number): string {
  const isInt = Number.isInteger(n);
  const [whole, frac] = (isInt ? `${n}` : n.toFixed(2)).split(".");
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, " ");
  return frac ? `${grouped},${frac}` : grouped;
}

/** Number grouping with commas (1,234,567.89). */
function groupComma(n: number): string {
  const isInt = Number.isInteger(n);
  const [whole, frac] = (isInt ? `${n}` : n.toFixed(2)).split(".");
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return frac ? `${grouped}.${frac}` : `${grouped}.00`;
}
