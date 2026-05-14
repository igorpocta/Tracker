/**
 * Tiny CSV helpers used by the Reports → Export CSV action.
 *
 * The format we emit is RFC 4180-ish: comma separated, fields containing
 * commas/quotes/newlines wrapped in double quotes, embedded quotes doubled.
 */

/** Escape a single CSV field. Returns the bare value when safe. */
export function csvEscape(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return "";
  const s = String(value);
  if (s === "") return "";
  // Quote if the value contains any of the characters that would break parsing.
  if (/[",\n\r]/.test(s)) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

/** Build a CSV string from a header row + body rows. */
export function buildCsv(
  header: string[],
  rows: (string | number | null | undefined)[][],
): string {
  const escaped: string[] = [];
  escaped.push(header.map(csvEscape).join(","));
  for (const row of rows) {
    escaped.push(row.map(csvEscape).join(","));
  }
  // Trailing newline keeps Excel happy on import.
  return `${escaped.join("\r\n")}\r\n`;
}

/**
 * Trigger a browser download of a CSV blob without leaving the page.
 *
 * Returns a cleanup function for tests; in real usage the temporary
 * `<a>` and object URL are torn down on the next event-loop tick.
 */
export function downloadCsv(filename: string, contents: string): void {
  if (typeof window === "undefined") return;
  const blob = new Blob([contents], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.style.display = "none";
  document.body.appendChild(anchor);
  anchor.click();
  window.setTimeout(() => {
    document.body.removeChild(anchor);
    URL.revokeObjectURL(url);
  }, 0);
}
