/**
 * Settings control for the global (system-wide) timer-toggle shortcut.
 *
 * Shows the current binding, lets the user record a new one (press the combo),
 * reset to the default, or disable it entirely. Registration is best-effort:
 * when the OS reports the combo as taken the backend returns
 * `registered: false` and we surface a warning so the user can pick another.
 */
import { LoaderCircle, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import {
  getGlobalShortcut,
  setGlobalShortcut,
  type GlobalShortcutStatus,
} from "../../api/commands";
import { useT } from "../../i18n";
import {
  DEFAULT_GLOBAL_SHORTCUT,
  eventToAccelerator,
  prettifyAccelerator,
} from "../../lib/shortcut";

const IS_MAC =
  typeof navigator !== "undefined" &&
  /mac/i.test(navigator.platform || navigator.userAgent || "");

export interface GlobalShortcutSettingProps {
  pushToast?: (kind: "error" | "success", message: string) => void;
}

export function GlobalShortcutSetting({ pushToast }: GlobalShortcutSettingProps) {
  const t = useT();
  const [status, setStatus] = useState<GlobalShortcutStatus | null>(null);
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getGlobalShortcut()
      .then((s) => {
        if (!cancelled) setStatus(s);
      })
      .catch(() => {
        if (!cancelled) setStatus({ accelerator: "", registered: false });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const persist = useCallback(
    async (accelerator: string) => {
      setBusy(true);
      setRecording(false);
      try {
        const next = await setGlobalShortcut(accelerator);
        setStatus(next);
        if (!next.registered && next.accelerator.trim().length > 0) {
          pushToast?.("error", t("common.shortcut.taken"));
        }
      } catch (e) {
        pushToast?.(
          "error",
          typeof e === "string" ? e : t("common.shortcut.saveFailed"),
        );
      } finally {
        setBusy(false);
      }
    },
    [pushToast, t],
  );

  // While recording, capture the next key combo from anywhere in the window.
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" || e.code === "Escape") {
        setRecording(false);
        return;
      }
      const accel = eventToAccelerator(e);
      if (accel) void persist(accel);
      // Incomplete combo (modifier only, or no strong modifier) → keep waiting.
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, persist]);

  const accelerator = status?.accelerator ?? "";
  const isDisabled = accelerator.trim().length === 0;
  const notRegistered = !isDisabled && status?.registered === false;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3 flex-wrap">
        <kbd
          data-testid="global-shortcut-current"
          className="min-w-[64px] text-center font-mono text-sm px-2.5 py-1.5
                     rounded-[var(--radius-md)] border border-[var(--border-default)]
                     bg-[var(--bg-hover)] text-[var(--text-primary)]"
        >
          {recording
            ? t("common.shortcut.press")
            : isDisabled
              ? t("common.shortcut.disabled")
              : prettifyAccelerator(accelerator, IS_MAC)}
        </kbd>

        {busy && (
          <LoaderCircle
            className="w-4 h-4 animate-spin text-[var(--text-tertiary)]"
            aria-hidden
          />
        )}

        {recording ? (
          <button
            type="button"
            onClick={() => setRecording(false)}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       text-[var(--text-secondary)] border border-[var(--border-default)]
                       hover:bg-[var(--bg-hover)] transition-colors duration-150"
          >
            {t("common.shortcut.cancel")}
          </button>
        ) : (
          <button
            type="button"
            onClick={() => setRecording(true)}
            disabled={busy}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm font-medium
                       text-[var(--text-primary)] border border-[var(--border-default)]
                       hover:bg-[var(--bg-hover)] transition-colors duration-150
                       disabled:opacity-60"
          >
            {t("common.shortcut.change")}
          </button>
        )}

        <button
          type="button"
          onClick={() => void persist(DEFAULT_GLOBAL_SHORTCUT)}
          disabled={busy || recording}
          className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                     text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]
                     transition-colors duration-150 disabled:opacity-60"
        >
          {t("common.shortcut.resetDefault")}
        </button>

        {!isDisabled && (
          <button
            type="button"
            onClick={() => void persist("")}
            disabled={busy || recording}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]
                       transition-colors duration-150 disabled:opacity-60"
          >
            {t("common.shortcut.disable")}
          </button>
        )}
      </div>

      {notRegistered && (
        <p
          role="alert"
          className="flex items-center gap-1.5 text-xs text-[var(--danger)]"
        >
          <TriangleAlert className="w-3.5 h-3.5 shrink-0" aria-hidden />
          {t("common.shortcut.notActive")}
        </p>
      )}

      <p className="text-[11px] text-[var(--text-tertiary)]">
        {t("common.shortcut.hint")}
      </p>
    </div>
  );
}
