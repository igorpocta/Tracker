/**
 * Helpers for the global timer-toggle shortcut.
 *
 * `eventToAccelerator` turns a browser `keydown` into a Tauri accelerator
 * string (the format `tauri-plugin-global-shortcut` and our backend
 * `set_global_shortcut` command understand). `prettifyAccelerator` renders a
 * stored accelerator back into something readable for the Settings UI.
 */

/** Default toggle accelerator; mirrors the backend `DEFAULT_GLOBAL_SHORTCUT`. */
export const DEFAULT_GLOBAL_SHORTCUT = "CommandOrControl+Shift+Period";

type KeyLike = Pick<
  KeyboardEvent,
  "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey"
>;

/** `event.code` values that are themselves a modifier — never a "main" key. */
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/** Punctuation `event.code` → Tauri key token (Tauri uses the `code` name). */
const PUNCTUATION = new Set([
  "Period",
  "Comma",
  "Slash",
  "Backslash",
  "Semicolon",
  "Quote",
  "BracketLeft",
  "BracketRight",
  "Minus",
  "Equal",
  "Backquote",
]);

const NAMED = new Set([
  "Space",
  "Enter",
  "Tab",
  "Backspace",
  "Delete",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

const ARROWS: Record<string, string> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

/** Map a `keydown` event's main key to a Tauri key token, or null if unusable. */
function mainKeyToken(code: string): string | null {
  if (code.startsWith("Key")) return code.slice(3); // KeyP -> P
  if (code.startsWith("Digit")) return code.slice(5); // Digit1 -> 1
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code; // F1..F24
  if (code in ARROWS) return ARROWS[code];
  if (PUNCTUATION.has(code)) return code;
  if (NAMED.has(code)) return code;
  return null;
}

/**
 * Build a Tauri accelerator string from a keydown event, or return `null` when
 * the combo is incomplete/unsuitable:
 *   - only a modifier key is held, or
 *   - the main key is unknown, or
 *   - there is no Cmd/Ctrl/Alt modifier (a bare or Shift-only key is too
 *     aggressive to grab system-wide).
 */
export function eventToAccelerator(e: KeyLike): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;

  const hasStrongModifier = e.metaKey || e.ctrlKey || e.altKey;
  if (!hasStrongModifier) return null;

  const key = mainKeyToken(e.code);
  if (!key) return null;

  const parts: string[] = [];
  // `CommandOrControl` resolves to ⌘ on macOS and Ctrl elsewhere at register
  // time, so one stored string works cross-platform.
  if (e.metaKey || e.ctrlKey) parts.push("CommandOrControl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

const MAC_GLYPHS: Record<string, string> = {
  CommandOrControl: "⌘",
  CmdOrCtrl: "⌘",
  Command: "⌘",
  Super: "⌘",
  Control: "⌃",
  Ctrl: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
};

const OTHER_LABELS: Record<string, string> = {
  CommandOrControl: "Ctrl",
  CmdOrCtrl: "Ctrl",
  Control: "Ctrl",
  Ctrl: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Super: "Win",
};

const KEY_GLYPHS: Record<string, string> = {
  Period: ".",
  Comma: ",",
  Slash: "/",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
};

/** Render a stored accelerator for display. Empty → "—" (disabled). */
export function prettifyAccelerator(accelerator: string, isMac: boolean): string {
  const trimmed = accelerator.trim();
  if (!trimmed) return "—";
  const tokens = trimmed.split("+").map((t) => {
    if (isMac && MAC_GLYPHS[t]) return MAC_GLYPHS[t];
    if (!isMac && OTHER_LABELS[t]) return OTHER_LABELS[t];
    return KEY_GLYPHS[t] ?? t;
  });
  return tokens.join(isMac ? "" : "+");
}
