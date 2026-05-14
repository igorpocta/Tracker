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

import { hasConfig } from "./api/commands";
import type { NavigateTarget } from "./api/types";
import { AppShell } from "./components/Layout/AppShell";
import { ErrorBoundary } from "./components/common/ErrorBoundary";
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

  if (initialRoute === null) {
    // Tiny pre-boot splash; the IPC call is fast enough that this rarely
    // flashes, but better than rendering the wrong screen for one frame.
    return <div aria-hidden className="min-h-screen" />;
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
 * Forwards backend-driven navigation events into React Router. Rendered inside
 * the router so it can call `useNavigate`.
 */
function NavigationBridge() {
  const navigate = useNavigate();

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    listen<NavigateTarget>("main-window:navigate", (event) => {
      const target = event.payload === "setup" ? "/setup" : "/";
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
