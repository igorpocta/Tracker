/**
 * Klient-side blok pro DevTools shortcut + kontextové menu v produkci.
 *
 * To NENÍ bezpečnostní opatření — kdokoliv s motivací si bundle rozbalí,
 * minifikaci přečte a tahle funkce ho nezastaví. Tady řešíme:
 *
 *   - F12 / Cmd+Opt+I / Ctrl+Shift+I / Cmd+Opt+J … neotevřou nic,
 *   - pravé tlačítko → "Inspect" zmizí (žádné context menu),
 *   - View Source (Cmd+U) je no-op.
 *
 * Smysl: ztížit náhodné prozkoumávání, nepoškodit dev zkušenost. Aktivní
 * jen v production buildu (`import.meta.env.PROD`); v `npm run dev` se
 * funkce nezavolá.
 */

const BLOCKED_KEYS = new Set(["F12"]);

function isInspectorShortcut(e: KeyboardEvent): boolean {
  if (BLOCKED_KEYS.has(e.key)) return true;
  const meta = e.metaKey || e.ctrlKey;
  if (!meta) return false;
  // Cmd/Ctrl + Shift + I / J / C → Chromium inspector
  if (e.shiftKey && (e.key === "I" || e.key === "J" || e.key === "C")) return true;
  // Cmd/Ctrl + Alt + I / J → Safari/Chromium developer tools
  if (e.altKey && (e.key === "I" || e.key === "J" || e.key === "i" || e.key === "j")) {
    return true;
  }
  // Cmd/Ctrl + U → View Source
  if (e.key === "u" || e.key === "U") return true;
  return false;
}

let installed = false;

export function installDevtoolsGuard(): void {
  // Tauri dev (`npm run dev`) má `import.meta.env.PROD === false`. Tj. v
  // tauri dev shell jsme stále PROD pouze pokud `tauri build` shell
  // bundl-uje s `mode=production`. Tj. dev workflow nedotčen.
  if (!import.meta.env.PROD) return;
  if (installed) return;
  installed = true;

  window.addEventListener(
    "keydown",
    (e) => {
      if (isInspectorShortcut(e)) {
        e.preventDefault();
        e.stopPropagation();
      }
    },
    { capture: true },
  );

  document.addEventListener(
    "contextmenu",
    (e) => {
      // Zachovat <input>/<textarea> context menu, ať user může copy/paste.
      const t = e.target as HTMLElement | null;
      const tag = t?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || t?.isContentEditable) {
        return;
      }
      e.preventDefault();
    },
    { capture: true },
  );
}
