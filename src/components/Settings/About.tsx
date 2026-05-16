/**
 * Settings → O aplikaci.
 *
 * Read-only panel s informacemi o aplikaci. Verze se taháme přímo z Tauri
 * runtime API (`getVersion`), takže nemusíme nikam manuálně psát konstantu —
 * `npm run version:bump` upraví `tauri.conf.json` a tahle obrazovka se
 * automaticky aktualizuje dalším buildem.
 */
import { getName, getTauriVersion, getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
}

export default function About() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getName(), getVersion(), getTauriVersion()])
      .then(([name, version, tauriVersion]) =>
        setInfo({ name, version, tauriVersion }),
      )
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
      });
  }, []);

  return (
    <div className="flex flex-col gap-6 w-full max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          O aplikaci
        </h2>
      </header>

      {error && (
        <p className="text-xs text-[var(--danger)]">
          Nepodařilo se načíst informace o aplikaci: {error}
        </p>
      )}

      {info && (
        <dl className="grid grid-cols-[140px_1fr] gap-y-2 text-sm">
          <dt className="text-[var(--text-tertiary)]">Název</dt>
          <dd className="text-[var(--text-primary)]">{info.name}</dd>

          <dt className="text-[var(--text-tertiary)]">Verze</dt>
          <dd className="text-[var(--text-primary)] font-mono tabular-nums">
            {info.version}
          </dd>

          <dt className="text-[var(--text-tertiary)]">Tauri</dt>
          <dd className="text-[var(--text-secondary)] font-mono tabular-nums">
            {info.tauriVersion}
          </dd>
        </dl>
      )}
    </div>
  );
}
