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

import { getFocusState, getPomodoroConfig } from "../api/commands";
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
      void notify(
        t("common.pomodoro.breakTitle"),
        t("common.pomodoro.breakBody", {
          workMin: cfg.work_min,
          breakMin: cfg.break_min,
        }),
      );

      breakTimerRef.current = window.setTimeout(
        () => {
          void notify(
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
 * Je zapnutý Focus mode s tlumením notifikací?
 *
 * Ptáme se až v okamžiku odeslání, ne při naplánování — pomodoro interval
 * běží desítky minut a uživatel mezitím Focus klidně zapne nebo vypne.
 * Systémové Nerušit umíme zapnout jen přes uživatelovu zkratku (a na Windows
 * vůbec), takže vlastní notifikace musíme potlačit sami.
 */
async function notificationsSuppressed(): Promise<boolean> {
  try {
    const state = await getFocusState();
    return state.active && state.block_notifications;
  } catch {
    // Mimo Tauri (testy, web build) nebo při restartu backendu — radši
    // notifikaci pošleme, než abychom ji tiše zahodili.
    return false;
  }
}

/**
 * Send a browser-level desktop notification. Tauri webview vystavuje Web
 * Notifications API, takže nepotřebujeme extra plugin.
 *
 * Pokud uživatel notifikace nepovolil, request si je rovnou vyžádá. Při
 * dalším volání už jen pošleme.
 */
async function notify(title: string, body: string) {
  if (typeof Notification === "undefined") return;
  if (await notificationsSuppressed()) return;
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
