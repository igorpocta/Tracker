/**
 * In-app keyboard shortcut wiring.
 *
 * These are *not* OS-global hotkeys (that would need
 * `tauri-plugin-global-shortcut`) — they only fire while a Tracker window
 * has focus. Good enough for the MVP: the user gets familiar shortcuts
 * without needing a permission prompt for system-wide capture.
 *
 * On macOS the modifier is the Command key (`event.metaKey`); on
 * Windows/Linux we accept Ctrl (`event.ctrlKey`). The detection is
 * runtime-based so the same build works on both platforms.
 */
import { useEffect } from "react";

/**
 * Returns `true` if the given keyboard event has the "primary" platform
 * modifier held — Cmd on macOS, Ctrl elsewhere.
 *
 * Exported for tests; consumers should normally use `useKeyboardShortcuts`.
 */
export function hasPrimaryModifier(event: KeyboardEvent): boolean {
  const isMac =
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || "");
  return isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

export interface KeyboardShortcutHandlers {
  /** Triggered by Cmd/Ctrl+R — typically "refresh cache". */
  onRefresh?: () => void;
  /** Triggered by Cmd/Ctrl+I — typically "re-index issue cache". */
  onReindex?: () => void;
  /** Triggered by Cmd/Ctrl+N — typically "new manual worklog entry". */
  onNewEntry?: () => void;
  /** Triggered by Cmd/Ctrl+, — typically "open settings". */
  onOpenSettings?: () => void;
}

/**
 * Wire a small set of in-app keyboard shortcuts to handler callbacks.
 *
 * The hook installs a single `keydown` listener on `window` (de-installed on
 * unmount) and dispatches to the appropriate handler based on the key.
 * Handlers are looked up via a ref-like closure so re-renders don't have to
 * re-attach the listener.
 */
export function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers): void {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!hasPrimaryModifier(event)) return;

      const key = event.key.toLowerCase();

      // Cmd/Ctrl+R — refresh.
      if (key === "r") {
        if (handlers.onRefresh) {
          event.preventDefault();
          handlers.onRefresh();
        }
        return;
      }

      // Cmd/Ctrl+I — re-index.
      if (key === "i") {
        if (handlers.onReindex) {
          event.preventDefault();
          handlers.onReindex();
        }
        return;
      }

      // Cmd/Ctrl+N — new entry.
      if (key === "n") {
        if (handlers.onNewEntry) {
          event.preventDefault();
          handlers.onNewEntry();
        }
        return;
      }

      // Cmd/Ctrl+, — settings.
      if (event.key === ",") {
        if (handlers.onOpenSettings) {
          event.preventDefault();
          handlers.onOpenSettings();
        }
        return;
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    handlers.onRefresh,
    handlers.onReindex,
    handlers.onNewEntry,
    handlers.onOpenSettings,
  ]);
}
