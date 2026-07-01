/**
 * Top-level React error boundary.
 *
 * Wraps the route tree so a render-time exception in any descendant doesn't
 * leave the user staring at a blank window. Shows a friendly fallback with
 * a "Reload" button (which simply calls `window.location.reload()` — the
 * WebView reloads the SPA, and the Tauri backend stays alive).
 *
 * Note: React error boundaries only catch render / lifecycle errors.
 * They do *not* catch errors thrown inside async handlers, event listeners,
 * or `setTimeout` callbacks. Those still need explicit try/catch at the
 * call site.
 */
import { Component, type ErrorInfo, type ReactNode } from "react";

import { translate } from "../../i18n";
import { usePrefsStore } from "../../stores/prefsStore";

interface ErrorBoundaryProps {
  /** Children to render when no error has been captured. */
  children: ReactNode;
  /**
   * Optional override of the fallback UI. Called with the captured error
   * and a `reset` callback that clears the boundary's error state.
   */
  fallback?: (error: Error, reset: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Surface the failure in the browser console so devs and users
    // sharing logs (Phase 10 telemetry) have something to grep for.
    // eslint-disable-next-line no-console
    console.error("[ErrorBoundary] render error:", error, info.componentStack);
  }

  reset = (): void => {
    this.setState({ error: null });
  };

  reload = (): void => {
    if (typeof window !== "undefined") {
      window.location.reload();
    }
  };

  render(): ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;

    if (this.props.fallback) {
      return this.props.fallback(error, this.reset);
    }

    const lang = usePrefsStore.getState().language;

    return (
      <div
        role="alert"
        className="min-h-screen flex items-center justify-center p-6 bg-[var(--bg-app)] text-[var(--text-primary)]"
      >
        <div className="max-w-md w-full bg-[var(--bg-surface)] border border-[var(--border-subtle)] rounded-[var(--radius-lg)] p-6 shadow-[var(--shadow-md)]">
          <h1 className="text-lg font-semibold mb-2">
            {translate(lang, "common.error.title")}
          </h1>
          <p className="text-sm text-[var(--text-secondary)] mb-4">
            {translate(lang, "common.error.body")}
          </p>
          {error.message && (
            <pre className="text-xs text-[var(--text-tertiary)] bg-[var(--bg-app)] rounded-[var(--radius-sm)] p-3 mb-4 overflow-auto max-h-32 border border-[var(--border-subtle)]">
              {error.message}
            </pre>
          )}
          <div className="flex gap-2 justify-end">
            <button
              type="button"
              onClick={this.reset}
              className="h-8 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border-default)] hover:bg-[var(--bg-hover)]"
            >
              {translate(lang, "common.error.tryAgain")}
            </button>
            <button
              type="button"
              onClick={this.reload}
              className="h-8 px-3 text-xs rounded-[var(--radius-md)] bg-[var(--accent)] hover:bg-[var(--accent-hover)] text-[var(--accent-text)]"
            >
              {translate(lang, "common.error.reload")}
            </button>
          </div>
        </div>
      </div>
    );
  }
}
