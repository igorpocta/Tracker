/**
 * `useClickOutside(ref, handler, enabled)` — fire `handler` on mousedown
 * outside the referenced DOM element.
 *
 * The listener is only installed while `enabled` is `true`, so callers
 * can wire it directly from their "dropdown is open" / "popover is open"
 * state without conditionally calling the hook (which would violate the
 * rules of hooks). When `enabled` is `false` the effect runs but the
 * cleanup is a no-op.
 *
 * Replaces the per-component `useEffect(() => { addEventListener(...) })`
 * boilerplate previously copy-pasted across `StartTrackingBar`,
 * `IssuePicker` and `AddEntryPanel`.
 */
import { useEffect, type RefObject } from "react";

export function useClickOutside<T extends HTMLElement>(
  ref: RefObject<T | null>,
  handler: () => void,
  enabled: boolean = true,
): void {
  useEffect(() => {
    if (!enabled) return;
    function onMouseDown(event: MouseEvent) {
      const el = ref.current;
      if (el && !el.contains(event.target as Node)) {
        handler();
      }
    }
    window.addEventListener("mousedown", onMouseDown);
    return () => window.removeEventListener("mousedown", onMouseDown);
  }, [ref, handler, enabled]);
}
