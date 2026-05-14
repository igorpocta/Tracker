/**
 * Thin React wrapper around `@tauri-apps/api/event::listen`.
 *
 * The Tauri `listen()` returns a promise of an `unlisten` function — handling
 * that lifecycle correctly with React effects is fiddly (you have to guard
 * against the listener resolving after the effect already unmounted). This
 * hook hides that boilerplate.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";

/**
 * Subscribe to a Tauri event. The `handler` is wrapped in a ref-less closure;
 * if you need it to read fresh state, prefer wrapping the body in a callback
 * that closes over state via the React closure (re-run effect on deps).
 *
 * @param eventName  Tauri event name (e.g. `"worklog-saved"`).
 * @param handler    Called with the event payload on each emit.
 */
export function useTauriEvent<T = unknown>(
  eventName: string,
  handler: (payload: T) => void,
): void {
  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    listen<T>(eventName, (event) => handler(event.payload))
      .then((u) => {
        if (cancelled) {
          u();
        } else {
          unlisten = u;
        }
      })
      .catch(() => {
        /* listening is best-effort outside Tauri (tests, web preview). */
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [eventName, handler]);
}
