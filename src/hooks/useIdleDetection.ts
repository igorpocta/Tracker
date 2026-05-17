/**
 * Idle detection — Toggl-style "byl jsi pryč, co s tím" workflow.
 *
 * Algoritmus (nově od bug-fixu #5):
 *  - Backend command `get_system_idle_seconds` vrací počet sekund od
 *    posledního systémového vstupu (CGEventSource na macOS, GetLastInputInfo
 *    na Windows). Měří se napříč celým OS, takže práce v IDE nebo prohlížeči
 *    se počítá jako aktivita.
 *  - Hook polluje každých 5 s. Když pollovaný idle ≥ threshold a timer běží,
 *    poznamená `idleStart = now - idleSecs`.
 *  - Když další poll vrátí idle < threshold (= uživatel se vrátil), vyhlásí
 *    "idle gap": rozdíl `now - idleStart`. Pak se předá callbacku, který
 *    otevře modal s volbami Keep / Discard / Discard & continue.
 *
 * Threshold se sdílí s `activity_threshold_min` (existující pref).
 *
 * Pre-refactor (2026-05): hook používal `window.addEventListener` na
 * `mousemove`/`keydown`, takže detekoval jen aktivitu uvnitř Tracker okna.
 * Uživatel pracující v jiné aplikaci dostával falešné idle dialogy po
 * návratu.
 */
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { getActivityThresholdMin, getSystemIdleSeconds } from "../api/commands";
import { useTimerStore } from "../stores/timerStore";

export interface IdleGap {
  /** Unix ms počátku ne-aktivity (= poslední registrovaná aktivita). */
  startedAtMs: number;
  /** Unix ms návratu (kdy uživatel zase něco udělal). */
  returnedAtMs: number;
  /** Délka ne-aktivity v sekundách. */
  durationSeconds: number;
}

/** Jak často se ptáme backendu na systémový idle čas. */
const POLL_INTERVAL_MS = 5_000;

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
  const thresholdSeconds = (thresholdQ.data ?? 5) * 60;

  const idleStartRef = useRef<number | null>(null);
  const [gap, setGap] = useState<IdleGap | null>(null);

  useEffect(() => {
    if (!active) {
      idleStartRef.current = null;
      return;
    }

    let cancelled = false;
    const tick = async () => {
      let idleSecs: number;
      try {
        idleSecs = await getSystemIdleSeconds();
      } catch {
        // Backend nedosažitelný (typicky v jsdom testech) — nic neděláme.
        return;
      }
      if (cancelled) return;

      const now = Date.now();
      const isIdle = idleSecs >= thresholdSeconds;

      if (isIdle) {
        // První detekce přechodu do idle: spočítej `startedAt` zpětně,
        // ať gap odpovídá reálnému času bez aktivity, ne až momentu pollu.
        if (idleStartRef.current === null) {
          idleStartRef.current = now - idleSecs * 1000;
        }
      } else if (idleStartRef.current !== null) {
        // Návrat z idle — vyhlásíme gap, pokud byl aspoň `threshold` dlouhý
        // (idle už klesl pod threshold, ale původní startedAt zachytí celou
        // dobu, takže porovnáme délku zpětně).
        const startedAtMs = idleStartRef.current;
        idleStartRef.current = null;
        const durationSeconds = Math.max(0, Math.round((now - startedAtMs) / 1000));
        if (durationSeconds >= thresholdSeconds) {
          setGap({
            startedAtMs,
            returnedAtMs: now,
            durationSeconds,
          });
        }
      }
    };

    // První kontrola hned, pak interval.
    void tick();
    const id = window.setInterval(() => {
      void tick();
    }, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [active, thresholdSeconds]);

  return {
    gap,
    dismiss: () => setGap(null),
  };
}
