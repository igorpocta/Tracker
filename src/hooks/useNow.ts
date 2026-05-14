/**
 * `useNow` — returns a `Date.now()` value that updates every `intervalMs`.
 *
 * Used by the live timer display so it ticks once per second without each
 * component needing its own `setInterval`. Components that don't care about
 * sub-minute precision can pass a larger interval.
 */
import { useEffect, useState } from "react";

export function useNow(intervalMs = 1000): number {
  const [now, setNow] = useState<number>(() => Date.now());

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);

  return now;
}
