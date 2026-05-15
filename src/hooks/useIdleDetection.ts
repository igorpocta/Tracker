/**
 * Idle detection — Toggl-style "byl jsi pryč, co s tím" workflow.
 *
 * Algoritmus:
 *  - `lastActivity` se aktualizuje při každém mousemove/keydown/mousedown.
 *  - Background interval (každých 5 s) kontroluje `now - lastActivity`. Pokud
 *    překročí threshold A timer běží, zaznamená `idleStart = lastActivity`.
 *  - Když přijde další input, hook detekuje `idleStart != null` a vyhlásí
 *    "idle gap": rozdíl `now - idleStart` v sekundách.
 *  - Gap se předá callbacku, který otevře modal s volbami:
 *    Keep / Discard / Discard & continue.
 *
 * Threshold se sdílí s `activity_threshold_min` (existující pref).
 */
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { getActivityThresholdMin } from "../api/commands";
import { useTimerStore } from "../stores/timerStore";

export interface IdleGap {
  /** Unix ms počátku ne-aktivity (= poslední registrovaná aktivita). */
  startedAtMs: number;
  /** Unix ms návratu (kdy uživatel zase něco udělal). */
  returnedAtMs: number;
  /** Délka ne-aktivity v sekundách. */
  durationSeconds: number;
}

export function useIdleDetection(): {
  gap: IdleGap | null;
  dismiss: () => void;
} {
  const active = useTimerStore((s) => s.active);
  const thresholdQ = useQuery({
    queryKey: ["activity-threshold-min"],
    queryFn: getActivityThresholdMin,
    staleTime: 60_000,
  });
  const thresholdMs = (thresholdQ.data ?? 5) * 60 * 1000;

  const lastActivityRef = useRef<number>(Date.now());
  const idleStartRef = useRef<number | null>(null);
  const [gap, setGap] = useState<IdleGap | null>(null);

  useEffect(() => {
    const onActivity = () => {
      const now = Date.now();
      // Detekce návratu z idle (pokud jsme byli déle pryč než threshold).
      if (
        idleStartRef.current !== null &&
        active &&
        now - idleStartRef.current >= thresholdMs
      ) {
        setGap({
          startedAtMs: idleStartRef.current,
          returnedAtMs: now,
          durationSeconds: Math.max(0, Math.round((now - idleStartRef.current) / 1000)),
        });
      }
      lastActivityRef.current = now;
      idleStartRef.current = null;
    };
    const events: (keyof WindowEventMap)[] = ["mousemove", "keydown", "mousedown"];
    events.forEach((e) =>
      window.addEventListener(e, onActivity, { passive: true }),
    );
    return () => {
      events.forEach((e) => window.removeEventListener(e, onActivity));
    };
  }, [active, thresholdMs]);

  // Background polling: pokud uplynul threshold bez aktivity a timer běží,
  // poznamenat `idleStart`. Jakmile přijde další event, onActivity to nasype
  // do `gap`.
  useEffect(() => {
    if (!active) {
      idleStartRef.current = null;
      return;
    }
    const id = window.setInterval(() => {
      const now = Date.now();
      if (
        idleStartRef.current === null &&
        now - lastActivityRef.current >= thresholdMs
      ) {
        idleStartRef.current = lastActivityRef.current;
      }
    }, 5_000);
    return () => window.clearInterval(id);
  }, [active, thresholdMs]);

  return {
    gap,
    dismiss: () => setGap(null),
  };
}
