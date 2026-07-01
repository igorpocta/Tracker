/**
 * Pomodoro notification driver.
 *
 * Když je aktivní časomíra a uživatel má zapnuto Pomodoro, hook nastaví
 * `setTimeout` na `work_min` minut. Jakmile timeout vyprší, pošle OS
 * notifikaci („Čas na pauzu") a naplánuje druhou notifikaci o `break_min`
 * minut později („Zpět do práce"). Když uživatel timer zastaví / přepne,
 * existující timeouts se zruší.
 *
 * Žádné lokální worklogy se neukládají — pomodoro je pure reminder loop.
 */
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { getPomodoroConfig } from "../api/commands";
import { queryKeys } from "../api/queryKeys";
import { useT } from "../i18n";
import { useTimerStore } from "../stores/timerStore";

export function usePomodoroTimer() {
  const t = useT();
  const active = useTimerStore((s) => s.active);
  const cfgQ = useQuery({
    queryKey: queryKeys.pomodoroConfig.all(),
    queryFn: getPomodoroConfig,
    staleTime: 60_000,
  });

  const workTimerRef = useRef<number | null>(null);
  const breakTimerRef = useRef<number | null>(null);

  useEffect(() => {
    const clear = () => {
      if (workTimerRef.current !== null) {
        window.clearTimeout(workTimerRef.current);
        workTimerRef.current = null;
      }
      if (breakTimerRef.current !== null) {
        window.clearTimeout(breakTimerRef.current);
        breakTimerRef.current = null;
      }
    };
    clear();

    const cfg = cfgQ.data;
    if (!cfg?.enabled || !active) return clear;

    // Zarovnání: pokud timer už nějakou dobu běží, vezmeme zbytek work cyklu.
    const elapsedMs = Date.now() - active.started_at;
    const workMs = cfg.work_min * 60_000;
    const remaining = Math.max(1_000, workMs - elapsedMs);

    workTimerRef.current = window.setTimeout(() => {
      notify(
        t("common.pomodoro.breakTitle"),
        t("common.pomodoro.breakBody", {
          workMin: cfg.work_min,
          breakMin: cfg.break_min,
        }),
      );

      breakTimerRef.current = window.setTimeout(
        () => {
          notify(
            t("common.pomodoro.workTitle"),
            t("common.pomodoro.workBody", { workMin: cfg.work_min }),
          );
        },
        cfg.break_min * 60_000,
      );
    }, remaining);

    return clear;
  }, [active, cfgQ.data, t]);
}

/**
 * Send a browser-level desktop notification. Tauri webview vystavuje Web
 * Notifications API, takže nepotřebujeme extra plugin.
 *
 * Pokud uživatel notifikace nepovolil, request si je rovnou vyžádá. Při
 * dalším volání už jen pošleme.
 */
function notify(title: string, body: string) {
  if (typeof Notification === "undefined") return;
  const send = () => {
    try {
      new Notification(title, { body });
    } catch {
      /* svg icon / construction may throw if permission revoked mid-call */
    }
  };
  if (Notification.permission === "granted") {
    send();
  } else if (Notification.permission !== "denied") {
    Notification.requestPermission()
      .then((perm) => {
        if (perm === "granted") send();
      })
      .catch(() => undefined);
  }
}
