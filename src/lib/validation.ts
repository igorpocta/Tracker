/**
 * Zod schemas used across forms.
 *
 * The setup wizard and Settings panes feed user-typed values through these
 * before invoking IPC commands. The Rust side re-validates so this layer is
 * "fast feedback, never authoritative", but keeping the two in lockstep means
 * the UI never reaches a state where a backend rejection surprises the user.
 *
 * Phase 18C — Item 23: extended with numeric range schemas matching the
 * backend `*_inner` validators. All messages are Czech (matching the rest of
 * the user-facing strings).
 */
import { z } from "zod";

/**
 * Jira Cloud instance URL. Must parse as a URL *and* use `https://` so we don't
 * accidentally accept `http://` or e.g. an `ftp://` scheme.
 */
export const urlSchema = z
  .string()
  .url("musí být platná URL")
  .regex(/^https:\/\//, "musí začínat https://");

/** Account email — basic shape check, not RFC-perfect. */
export const emailSchema = z.string().email("musí být platný e-mail");

/**
 * Jira API token. We can't actually verify the token without hitting the API,
 * but we can at least reject obviously-too-short strings.
 */
export const tokenSchema = z.string().min(10, "API token vypadá příliš krátký");

// -----------------------------------------------------------------------------
// Numeric setting schemas (Phase 18C — Item 23)
// -----------------------------------------------------------------------------

/**
 * Hourly rate. Must be a finite non-negative number ≤ 100 000. Matches
 * `set_hourly_rate_inner` on the Rust side.
 *
 * The schema accepts numbers, not strings — call sites that read from `<input
 * type="text">` should coerce via `Number(value)` (and reject NaN before
 * calling parse, since Zod treats NaN as a number).
 */
export const hourlyRateSchema = z
  .number()
  .refine((n) => Number.isFinite(n), "musí být platné číslo")
  .refine((n) => n >= 0, "nesmí být záporná")
  .refine((n) => n <= 100_000, "je příliš vysoká (max 100 000)");

/**
 * Daily goal in hours. 0.5..=24, matching the backend.
 */
export const dailyGoalHoursSchema = z
  .number()
  .refine((n) => Number.isFinite(n), "musí být platné číslo")
  .refine((n) => n >= 0.5, "minimum je 0,5 h")
  .refine((n) => n <= 24, "maximum je 24 h");

/**
 * Activity threshold in minutes. 1..=120 — backend caps at 120 to give some
 * head-room versus the documented 1..=60 in the spec.
 */
export const activityThresholdSchema = z
  .number()
  .int("musí být celé číslo")
  .min(1, "minimum je 1 min")
  .max(120, "maximum je 120 min");

/** Rounding interval — strict enum of valid steps. */
export const roundingIntervalSchema = z
  .number()
  .refine((n) => [1, 5, 15, 60].includes(n), "musí být 1, 5, 15 nebo 60");

/** Working week mask — 7-bit value (Mon=1 … Sun=64). */
export const workingWeekMaskSchema = z
  .number()
  .int("musí být celé číslo")
  .min(0, "minimum je 0")
  .max(127, "maximum je 127");

/** Goal-slider hours (UI slider in Settings → Cíle). */
export const goalSliderHoursSchema = z
  .number()
  .refine((n) => Number.isFinite(n), "musí být platné číslo")
  .min(1, "minimum je 1 h")
  .max(14, "maximum je 14 h");

/**
 * HH:MM 24-hour time. Accepts "0:00" through "23:59". Used by the start-time
 * widget in the StopDialog / manual worklog form.
 */
export const timeOfDaySchema = z
  .string()
  .regex(/^([01]?\d|2[0-3]):[0-5]\d$/, "musí být ve formátu HH:MM");

/** Jira issue key — uppercase project, hyphen, positive integer (no leading 0). */
export const issueKeySchema = z
  .string()
  .regex(
    /^[A-Z][A-Z0-9]+-[1-9][0-9]*$/,
    "musí být ve formátu PROJ-123",
  );

/** JQL query — non-empty after trim, max 2000 chars, no NUL bytes. */
export const jqlSchema = z
  .string()
  .refine((s) => s.trim().length > 0, "dotaz nesmí být prázdný")
  .refine((s) => s.length <= 2000, "dotaz je příliš dlouhý (max 2000 znaků)")
  .refine((s) => !s.includes("\0"), "obsahuje neplatný znak");

/** ISO-4217 currency code — enum of allowed values, mirrors the backend. */
export const currencySchema = z.enum(
  ["CZK", "EUR", "USD", "GBP", "PLN", "CHF"],
);

/**
 * Parse a user-typed rate string into a number we can hand to
 * `hourlyRateSchema`. Treats `,` as the decimal separator (Czech locale),
 * trims whitespace, returns `null` for the blank/zero-equivalent input so
 * the caller can choose to disable the earnings card.
 *
 * Crucially: rejects exponent notation (`2e99`) and any leading-sign hijinks
 * so the value can't surface as `Infinity` after `Number()`.
 */
export function parseRateInput(raw: string): number | null {
  const trimmed = raw.trim();
  if (trimmed === "") return 0;
  // Disallow scientific notation — these almost always come from a typo and
  // the result of `Number()` is `Infinity` for large exponents.
  if (/[eE]/.test(trimmed)) return null;
  const normalised = trimmed.replace(",", ".");
  // Allow optional sign + digits + optional fraction. Anything else is junk.
  if (!/^-?\d+(\.\d+)?$/.test(normalised)) return null;
  const n = Number(normalised);
  if (!Number.isFinite(n)) return null;
  return n;
}

/**
 * Convenience: returns the first validation error message produced by `schema`
 * against `value`, or `null` if the value is valid. Handy for inline form UX.
 */
export function firstError<T>(schema: z.ZodType<T>, value: unknown): string | null {
  const result = schema.safeParse(value);
  if (result.success) return null;
  return result.error.issues[0]?.message ?? "neplatná hodnota";
}
