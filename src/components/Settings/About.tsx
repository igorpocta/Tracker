/**
 * Settings → O aplikaci.
 *
 * Read-only panel s informacemi o aplikaci. Verze se taháme přímo z Tauri
 * runtime API (`getVersion`), takže nemusíme nikam manuálně psát konstantu —
 * `npm run version:bump` upraví `tauri.conf.json` a tahle obrazovka se
 * automaticky aktualizuje dalším buildem. Commit hash se zapéká přes
 * `vite.config.ts` (`__COMMIT_HASH__`).
 */
import { getName, getTauriVersion, getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

import { openUrl } from "../../api/commands";

// Inline GitHub mark SVG — lucide-react 1.16 (currently pinned) doesn't
// ship a `Github` icon and we don't want to bump the icon library just
// for this single glyph. Path is the official Octocat mark, simplified.
function GithubMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      aria-hidden
      className={className}
      fill="currentColor"
    >
      <path d="M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.1.79-.25.79-.56v-2.1c-3.2.7-3.87-1.36-3.87-1.36-.52-1.32-1.27-1.67-1.27-1.67-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.02 1.75 2.68 1.24 3.34.95.1-.74.4-1.25.72-1.54-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.28 1.18-3.09-.12-.29-.51-1.46.11-3.04 0 0 .96-.31 3.14 1.18a10.94 10.94 0 0 1 5.72 0c2.18-1.49 3.14-1.18 3.14-1.18.62 1.58.23 2.75.11 3.04.73.81 1.18 1.83 1.18 3.09 0 4.42-2.7 5.39-5.27 5.68.41.36.78 1.06.78 2.14v3.18c0 .31.21.66.8.55C20.21 21.39 23.5 17.08 23.5 12 23.5 5.65 18.35.5 12 .5Z" />
    </svg>
  );
}

const GITHUB_REPO_URL = "https://github.com/igorpocta/Tracker";

interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
}

export default function About() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const commitHash =
    typeof __COMMIT_HASH__ !== "undefined" ? __COMMIT_HASH__ : "unknown";

  useEffect(() => {
    Promise.all([getName(), getVersion(), getTauriVersion()])
      .then(([name, version, tauriVersion]) =>
        setInfo({ name, version, tauriVersion }),
      )
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
      });
  }, []);

  const openGithub = () => {
    openUrl(GITHUB_REPO_URL).catch((e) =>
      console.error("[About] open GitHub failed:", e),
    );
  };

  return (
    <div className="flex flex-col gap-6 w-full">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          O aplikaci
        </h2>
      </header>

      <p className="text-sm leading-relaxed text-[var(--text-secondary)]">
        Tracker je lokální desktopový time-tracker pro Jira Cloud a Freelo.
        Spouští časomíry, zaznamenává worklogy do lokální SQLite databáze
        a synchronizuje je do připojených systémů. Všechna data zůstávají
        u vás na počítači — bez cloudu, bez účtu, bez telemetrie (krom
        volitelného anonymního reportu chyb).
      </p>

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

          <dt className="text-[var(--text-tertiary)]">Commit</dt>
          <dd className="text-[var(--text-secondary)] font-mono tabular-nums">
            {commitHash}
          </dd>

          <dt className="text-[var(--text-tertiary)]">Tauri</dt>
          <dd className="text-[var(--text-secondary)] font-mono tabular-nums">
            {info.tauriVersion}
          </dd>
        </dl>
      )}

      <div className="pt-2 border-t border-[var(--border-subtle)]">
        <button
          type="button"
          onClick={openGithub}
          title="Otevřít repozitář na GitHubu"
          className="inline-flex items-center gap-2 h-8 px-3 rounded-[var(--radius-md)]
                     text-xs text-[var(--text-secondary)]
                     border border-[var(--border-subtle)]
                     hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]
                     transition-colors duration-150"
        >
          <GithubMark className="w-3.5 h-3.5" />
          GitHub repozitář
        </button>
      </div>
    </div>
  );
}
