/**
 * `useEscapeKey(handler, enabled)` — fire `handler` on global Escape
 * keydown.
 *
 * Same pattern as [`useClickOutside`]: only installed while `enabled`
 * is `true`. Centralises the "close on Escape" boilerplate that several
 * dropdown / picker components were repeating.
 *
 * NOTE: this is a window-level listener; it does NOT respect input
 * focus. Components that need Escape to also clear a typed value
 * before closing (e.g. the inline edit cells in `WorklogRow`) keep
 * doing it on their own `onKeyDown` because they need access to the
 * input element to call `.blur()`.
 */
import { useEffect } from "react";

export function useEscapeKey(handler: () => void, enabled: boolean = true): void {
  useEffect(() => {
    if (!enabled) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        handler();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handler, enabled]);
}
