/**
 * Zod schemas used by the setup wizard.
 *
 * Kept deliberately small: each field gets one schema with a friendly message
 * so the UI can surface validation errors directly via `safeParse`.
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

/**
 * Convenience: returns the first validation error message produced by `schema`
 * against `value`, or `null` if the value is valid. Handy for inline form UX.
 */
export function firstError<T>(schema: z.ZodType<T>, value: unknown): string | null {
  const result = schema.safeParse(value);
  if (result.success) return null;
  return result.error.issues[0]?.message ?? "neplatná hodnota";
}
