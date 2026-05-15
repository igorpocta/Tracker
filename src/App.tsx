import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import {
  MemoryRouter,
  Navigate,
  Route,
  Routes,
  useNavigate,
} from "react-router-dom";

import { getInstallId, getSentryEnabled, hasConfig } from "./api/commands";
import type { NavigateTarget } from "./api/types";
import { AppShell } from "./components/Layout/AppShell";
import { ErrorBoundary } from "./components/common/ErrorBoundary";
import { initSentry } from "./lib/sentry";
import Audit from "./routes/Audit";
import Calendar from "./routes/Calendar";
import Goals from "./routes/Goals";
import Reports from "./routes/Reports";
import Settings from "./routes/Settings";
import Setup from "./routes/Setup";
import TimeLog from "./routes/TimeLog";

/**
 * Shared QueryClient instance. Sensible defaults for a desktop app:
 *   - No automatic refocus refetches (we drive invalidation via Tauri
 *     events instead).
 *   - `staleTime: 30s` so navigating between routes doesn't refetch.
 */
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      staleTime: 30_000,
      retry: 1,
    },
  },
});

/**
 * Top-level app shell.
 *
 * Two concerns:
 * 1. On mount, ask the backend whether we already have a usable config and
 *    pick `/` or `/setup` as the initial route accordingly.
 * 2. Subscribe to the `main-window:navigate` Tauri event so the backend can
 *    drive navigation (used e.g. by the tray menu's "Settings" item).
 *
 * We use `MemoryRouter` instead of `BrowserRouter` so the WebView's URL bar
 * stays clean and we don't have to fight `file://` semantics during dev.
 */
export default function App() {
  const [initialRoute, setInitialRoute] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    hasConfig()
      .then((ok) => {
        if (!cancelled) setInitialRoute(ok ? "/" : "/setup");
      })
      .catch(() => {
        if (!cancelled) setInitialRoute("/setup");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Phase 19 — opt-in Sentry. Runs in parallel with the route decision so
  // boot is never blocked on the toggle lookup. The SDK init itself is a
  // no-op if the user hasn't opted in OR no DSN is configured at build
  // time, so failing-soft here is correct.
  useEffect(() => {
    let cancelled = false;
    Promise.all([getSentryEnabled(), getInstallId()])
      .then(([enabled, installId]) => {
        if (cancelled || !enabled) return;
        initSentry({ installId });
      })
      .catch(() => {
        /* non-Tauri context or backend mid-restart — Sentry stays off. */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (initialRoute === null) {
    // Phase 18B — Item 19: centered loading splash so the brief boot
    // window doesn't flash an empty viewport. Spinner uses the accent so
    // hot-reloading colors are visible even pre-hydration.
    return <BootSplash />;
  }

  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={[initialRoute]}>
          <NavigationBridge />
          <Routes>
            {/* Setup wizard is rendered outside the AppShell so it owns the
                full window. */}
            <Route path="/setup" element={<Setup />} />
            {/* Everything else lives inside the shell. */}
            <Route element={<AppShell />}>
              <Route index element={<TimeLog />} />
              <Route path="/reports" element={<Reports />} />
              <Route path="/calendar" element={<Calendar />} />
              <Route path="/goals" element={<Goals />} />
              <Route path="/audit" element={<Audit />} />
              <Route path="/settings" element={<Settings />} />
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

/**
 * Phase 18B — Item 19: centred Tracker boot splash.
 *
 * We can't render an arbitrarily-themed surface because the prefs store
 * hasn't loaded yet — so we lean on the CSS variables that are already set
 * by `index.css` for the default theme. Falling back gracefully here keeps
 * the layout calm even if hydration hangs for a beat.
 */
function BootSplash() {
  return (
    <div
      aria-busy
      className="min-h-screen w-full flex flex-col items-center justify-center gap-3"
      style={{ background: "var(--bg-app)", color: "var(--text-primary)" }}
    >
      <div
        className="text-[44px] leading-none italic font-semibold select-none"
        style={{
          color: "var(--accent)",
          fontFamily: "var(--font-script), serif",
        }}
      >
        Tracker.
      </div>
      <div className="flex items-center gap-2 text-[var(--text-tertiary)] text-xs">
        <span
          className="inline-block w-3 h-3 rounded-full animate-spin"
          style={{
            border: "2px solid var(--accent-soft)",
            borderTopColor: "var(--accent)",
          }}
          aria-hidden
        />
        Načítání…
      </div>
    </div>
  );
}

/**
 * Forwards backend-driven navigation events into React Router. Rendered inside
 * the router so it can call `useNavigate`.
 */
function NavigationBridge() {
  const navigate = useNavigate();

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    listen<NavigateTarget>("main-window:navigate", (event) => {
      const target =
        event.payload === "setup"
          ? "/setup"
          : event.payload === "settings"
            ? "/settings"
            : "/";
      navigate(target, { replace: true });
    })
      .then((u) => {
        if (cancelled) {
          u();
        } else {
          unlisten = u;
        }
      })
      .catch(() => {
        /* listening is best-effort in non-Tauri contexts (tests, web build). */
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [navigate]);

  return null;
}
