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

    return (
      <div
        role="alert"
        className="min-h-screen flex items-center justify-center p-6 bg-[#0f0f0f] text-neutral-100"
      >
        <div className="max-w-md w-full bg-neutral-900/80 border border-neutral-800 rounded-lg p-6 shadow-lg">
          <h1 className="text-lg font-semibold mb-2">Something went wrong</h1>
          <p className="text-sm text-neutral-400 mb-4">
            Tracker hit an unexpected error and couldn't continue. Reloading
            the window will usually fix it.
          </p>
          {error.message && (
            <pre className="text-xs text-neutral-500 bg-black/40 rounded p-3 mb-4 overflow-auto max-h-32">
              {error.message}
            </pre>
          )}
          <div className="flex gap-2 justify-end">
            <button
              type="button"
              onClick={this.reset}
              className="px-3 py-1.5 text-xs rounded border border-neutral-700 hover:bg-neutral-800"
            >
              Try again
            </button>
            <button
              type="button"
              onClick={this.reload}
              className="px-3 py-1.5 text-xs rounded bg-sky-600 hover:bg-sky-500 text-white"
            >
              Reload
            </button>
          </div>
        </div>
      </div>
    );
  }
}
