/**
 * Thin React wrapper around `@tauri-apps/api/event::listen`.
 *
 * The Tauri `listen()` returns a promise of an `unlisten` function — handling
 * that lifecycle correctly with React effects is fiddly (you have to guard
 * against the listener resolving after the effect already unmounted). This
 * hook hides that boilerplate.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/**
 * Subscribe to a Tauri event for the lifetime of the component, dispatching to
 * the LATEST `handler` on each emit.
 *
 * The handler is held in a ref so the listener subscribes exactly once per
 * `eventName` — it does NOT re-subscribe when the caller passes a fresh inline
 * handler each render. Re-subscribing was both wasteful and unsafe: `listen()`
 * is async, so between the synchronous teardown and the new listener resolving
 * there was a window with no live listener, and a component that re-renders
 * every second (e.g. a running-timer tick) could drop events that fired in it.
 *
 * @param eventName  Tauri event name (e.g. `"worklog-saved"`).
 * @param handler    Called with the event payload on each emit.
 */
export function useTauriEvent<T = unknown>(
  eventName: string,
  handler: (payload: T) => void,
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    listen<T>(eventName, (event) => handlerRef.current(event.payload))
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
  }, [eventName]);
}
