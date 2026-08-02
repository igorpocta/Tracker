/**
 * Focus-mode banner renderer.
 *
 *   ┌──────────────────────────────────────┐
 *   │ ⛨  Focus mode                        │
 *   │    Slack je během Focusu blokovaná.  │
 *   └──────────────────────────────────────┘
 *
 * The window itself is created, positioned, shown and hidden by
 * `src-tauri/src/focus/overlay.rs`; this module only paints what the backend
 * pushes over `focus-overlay:notice`. It is click-through, so there is
 * deliberately nothing interactive here.
 */
import { ShieldBan } from "lucide-react";
import { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import { getAccentColor, getFocusOverlayNotice, getTheme } from "./api/commands";
import type { FocusOverlayNotice, ThemePref } from "./api/types";
import { useT } from "./i18n";
import { useTauriEvent } from "./hooks/useTauriEvent";
import { applyPalette } from "./lib/accent";

import "./index.css";

function applyThemeAttr(theme: ThemePref): void {
  if (typeof document === "undefined") return;
  const html = document.documentElement;
  if (theme === "auto") {
    html.removeAttribute("data-theme");
  } else {
    html.setAttribute("data-theme", theme);
  }
}

/** Match the main window's theme + palette so the banner doesn't look foreign. */
async function hydrateAppearance(): Promise<void> {
  try {
    const [theme, accent] = await Promise.all([
      getTheme().catch<ThemePref>(() => "auto"),
      getAccentColor().catch(() => "aurora"),
    ]);
    applyThemeAttr(theme);
    applyPalette(accent);
  } catch {
    /* defaults are fine */
  }
}

export function FocusOverlay() {
  const t = useT();
  const [notice, setNotice] = useState<FocusOverlayNotice | null>(null);

  useEffect(() => {
    void hydrateAppearance();
    // This window is built the first time a session blocks something, so the
    // event announcing that first block is emitted before React is listening.
    // Ask for it instead of showing a generic banner.
    getFocusOverlayNotice()
      .then((current) => {
        if (current) setNotice((shown) => shown ?? current);
      })
      .catch(() => {
        /* non-Tauri context — the generic banner is fine */
      });
  }, []);

  useTauriEvent<FocusOverlayNotice>("focus-overlay:notice", (payload) => {
    if (payload) setNotice(payload);
  });

  const message = notice
    ? t(notice.killed ? "focus.overlay.killed" : "focus.overlay.hidden", {
        app: notice.app_name,
      })
    : t("focus.running");

  return (
    <div
      className="h-full w-full flex items-center gap-3 px-4"
      style={{
        background: "var(--bg-surface)",
        color: "var(--text-primary)",
        borderRadius: 12,
        boxShadow: "var(--shadow-popover)",
        outline: "0.5px solid var(--border-default)",
        outlineOffset: "-0.5px",
      }}
    >
      <span
        className="shrink-0 w-9 h-9 rounded-full flex items-center justify-center"
        style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
      >
        <ShieldBan className="w-5 h-5" aria-hidden />
      </span>
      <div className="min-w-0">
        <div className="text-[13px] font-semibold leading-tight">{t("focus.title")}</div>
        <div className="text-[12px] leading-snug text-[var(--text-secondary)] truncate">
          {message}
        </div>
      </div>
    </div>
  );
}

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(<FocusOverlay />);
}
