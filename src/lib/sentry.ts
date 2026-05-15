/**
 * Phase 19 — opt-in Sentry error reporting (frontend half).
 *
 * Initialisation gates:
 *   1. A DSN must be configured. Two sources, in order:
 *      a. Build-time embed via `import.meta.env.VITE_TRACKER_SENTRY_DSN_FRONTEND`
 *         (set `TRACKER_SENTRY_DSN_FRONTEND=<dsn>` before `npm run build`).
 *      b. Runtime override passed to `initSentry({ dsn })` — used when the
 *         backend forwards an env var the bundle wasn't built with.
 *   2. The user must have opted in (`get_sentry_enabled() -> true`).
 *
 * Privacy guarantees:
 *   * `beforeSend` and `beforeBreadcrumb` callbacks redact anything that
 *     smells like a token, API key, secret, password, cookie, or
 *     Authorization header before the event leaves the device.
 *   * Email addresses are masked to keep only the first letter of the
 *     local part (`igor.pocta@example.com` → `i***@example.com`).
 *   * `replayIntegration` is configured with `maskAllText` + `blockAllMedia`
 *     and `replaysSessionSampleRate: 0` so we ONLY capture replay frames
 *     leading up to a captured error — never an arbitrary session.
 *
 * The shutdown path (`shutdownSentry()`) is wired to the Settings toggle:
 * when the user disables reporting, the frontend SDK is flushed within
 * 2 seconds and stops capturing immediately.
 */
import * as Sentry from "@sentry/react";

/**
 * Build-time DSN injected by Vite. `undefined` (or empty string) means
 * the user / packager didn't configure one — we'll fall back to a
 * runtime-supplied DSN, and otherwise stay off.
 */
const BUILD_DSN: string | undefined =
  ((import.meta.env.VITE_TRACKER_SENTRY_DSN_FRONTEND as string | undefined) || undefined) ||
  undefined;

let initialized = false;

export interface SentryInitArgs {
  /** Optional explicit DSN override; falls back to the build-time one. */
  dsn?: string | null;
  /** Stable per-install anonymous id (UUID v4 generated and persisted by
   *  the backend). Used as Sentry's `user.id` so events from one install
   *  group together without exposing PII. */
  installId?: string | null;
}

/**
 * Initialise the Sentry SDK. Idempotent — a second call is a no-op.
 * Returns `true` when Sentry is now active, `false` when no DSN was
 * available.
 */
export function initSentry({ dsn, installId }: SentryInitArgs): boolean {
  if (initialized) return true;
  const effectiveDsn = (dsn && dsn.length > 0 ? dsn : BUILD_DSN) || null;
  if (!effectiveDsn) return false;

  // `__APP_VERSION__` is injected by `vite.config.ts#define`. In tests we
  // fall back to a literal so the import doesn't blow up under Vitest
  // (which doesn't run the define pass).
  const version =
    typeof __APP_VERSION__ !== "undefined" ? __APP_VERSION__ : "dev";

  Sentry.init({
    dsn: effectiveDsn,
    release: `tracker@${version}`,
    environment: import.meta.env.MODE,
    integrations: [
      Sentry.browserTracingIntegration(),
      Sentry.replayIntegration({ maskAllText: true, blockAllMedia: true }),
    ],
    tracesSampleRate: 0.1,
    replaysSessionSampleRate: 0,
    replaysOnErrorSampleRate: 1.0,
    sendDefaultPii: false,
    beforeSend: scrubEvent,
    beforeBreadcrumb: scrubBreadcrumb,
  });

  if (installId) Sentry.setUser({ id: installId });
  Sentry.setTag("app.version", version);
  initialized = true;
  return true;
}

/**
 * Flush + close the SDK. Called from the Settings toggle when the user
 * turns reporting off, so capture stops within the lifetime of the
 * current process (not just on restart).
 */
export async function shutdownSentry(): Promise<void> {
  if (!initialized) return;
  await Sentry.close(2000);
  initialized = false;
}

/** Whether `initSentry` has already wired up the SDK. */
export function isSentryInitialized(): boolean {
  return initialized;
}

// ---------------------------------------------------------------------------
// Scrubbing
// ---------------------------------------------------------------------------

/**
 * Drop / redact secrets and PII from an outgoing event. Best-effort
 * defence-in-depth — Sentry strips some PII when `sendDefaultPii: false`,
 * but we layer extra guards for Jira tokens, API keys, and emails.
 */
export function scrubEvent(
  event: Sentry.ErrorEvent,
): Sentry.ErrorEvent | null {
  if (event.request) {
    if (event.request.cookies) delete event.request.cookies;
    if (event.request.headers) {
      for (const k of Object.keys(event.request.headers)) {
        if (headerNameIsSensitive(k)) {
          event.request.headers[k] = "[redacted]";
          continue;
        }
        const v = event.request.headers[k];
        if (typeof v === "string" && headerValueIsSensitive(v)) {
          event.request.headers[k] = "[redacted]";
        }
      }
    }
    // Strip any query string entirely if it looks like it carries auth.
    if (
      typeof event.request.query_string === "string" &&
      looksLikeSecretFragment(event.request.query_string)
    ) {
      event.request.query_string = "[redacted]";
    }
  }
  if (event.user?.email) event.user.email = maskEmail(event.user.email);
  if (event.user) {
    // Backend never sends real identifiers other than the anonymous
    // install id we set in `initSentry`; drop everything else.
    delete event.user.username;
    delete event.user.ip_address;
  }
  scrubObject(event.extra);
  scrubObject(event.contexts);
  scrubObject(event.tags);
  return event;
}

export function scrubBreadcrumb(
  b: Sentry.Breadcrumb,
): Sentry.Breadcrumb | null {
  if (b.data) scrubObject(b.data);
  if (typeof b.message === "string" && looksLikeSecretFragment(b.message)) {
    b.message = "[redacted]";
  }
  return b;
}

/**
 * Recursively walk an object and redact entries whose keys look like
 * secrets, or whose string values look like opaque tokens.
 *
 * Exported for unit testing.
 */
export function scrubObject(o: unknown): void {
  if (!o || typeof o !== "object") return;
  const obj = o as Record<string, unknown>;
  for (const key of Object.keys(obj)) {
    if (keyIsSensitive(key)) {
      obj[key] = "[redacted]";
      continue;
    }
    const v = obj[key];
    if (typeof v === "string" && looksLikeToken(v)) {
      obj[key] = "[redacted-token]";
    } else if (v && typeof v === "object") {
      scrubObject(v);
    }
  }
}

function keyIsSensitive(name: string): boolean {
  const lower = name.toLowerCase();
  return (
    lower.includes("token") ||
    lower.includes("api_key") ||
    lower.includes("api-key") ||
    lower.includes("apikey") ||
    lower.includes("secret") ||
    lower.includes("password") ||
    lower.includes("cookie") ||
    lower.includes("authorization")
  );
}

function headerNameIsSensitive(name: string): boolean {
  const lower = name.toLowerCase();
  return (
    lower === "authorization" ||
    lower === "cookie" ||
    lower === "set-cookie" ||
    lower === "proxy-authorization" ||
    lower.includes("token") ||
    lower.includes("api-key") ||
    lower.includes("apikey")
  );
}

function headerValueIsSensitive(value: string): boolean {
  const trimmed = value.trimStart();
  return (
    trimmed.startsWith("Bearer ") ||
    trimmed.startsWith("Basic ") ||
    trimmed.startsWith("Digest ")
  );
}

function looksLikeSecretFragment(s: string): boolean {
  const lower = s.toLowerCase();
  return (
    lower.includes("token=") ||
    lower.includes("apikey=") ||
    lower.includes("api_key=") ||
    lower.includes("password=") ||
    lower.includes("authorization:")
  );
}

/**
 * Heuristic for "this string is probably an API token": 20–200 chars of
 * base64 / hex / identifier characters with no whitespace.
 */
export function looksLikeToken(s: string): boolean {
  if (s.length < 20 || s.length > 200) return false;
  return /^[A-Za-z0-9_\-.=]+$/.test(s);
}

/** Mask the local part of an email address. */
export function maskEmail(e: string): string {
  const at = e.indexOf("@");
  if (at < 0) return "[redacted]";
  const local = e.slice(0, at);
  const masked = local.length > 1 ? `${local[0]}***` : "***";
  return `${masked}${e.slice(at)}`;
}

/** Re-export Sentry's `ErrorBoundary` for nested catch-zones. */
export const SentryErrorBoundary = Sentry.ErrorBoundary;
