/**
 * Settings → About tab.
 *
 * Static info card with version + a few useful links.
 */
import { ExternalLink, Heart } from "lucide-react";

import { openUrl } from "../../api/commands";

export default function About() {
  const platform =
    typeof navigator !== "undefined" ? navigator.platform || "unknown" : "unknown";
  const userAgent =
    typeof navigator !== "undefined" ? navigator.userAgent : "unknown";

  return (
    <div className="flex flex-col gap-6 max-w-xl text-sm">
      <section>
        <h3 className="text-lg font-semibold text-[var(--text-primary)]">Tracker</h3>
        <p className="text-xs text-[var(--text-secondary)] mt-1">
          A keyboard-friendly Jira time tracker.
        </p>
      </section>

      <dl className="grid grid-cols-3 gap-x-3 gap-y-2 text-xs">
        <dt className="text-[var(--text-tertiary)]">Version</dt>
        <dd className="col-span-2 text-[var(--text-primary)] font-mono">0.1.0</dd>

        <dt className="text-[var(--text-tertiary)]">Bundle id</dt>
        <dd className="col-span-2 text-[var(--text-primary)] font-mono">
          com.tracker.app
        </dd>

        <dt className="text-[var(--text-tertiary)]">Platform</dt>
        <dd className="col-span-2 text-[var(--text-primary)]">{platform}</dd>

        <dt className="text-[var(--text-tertiary)]">User agent</dt>
        <dd className="col-span-2 text-[var(--text-tertiary)] break-words text-[11px]">
          {userAgent}
        </dd>
      </dl>

      <section className="border-t border-[var(--border-subtle)] pt-4 flex flex-col gap-2">
        <h3 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
          Links
        </h3>
        <ul className="flex flex-col gap-1 text-xs">
          <li>
            <button
              type="button"
              onClick={() => {
                openUrl("https://www.atlassian.com/software/jira").catch(() => {
                  /* ignore */
                });
              }}
              className="text-[var(--accent)] hover:underline inline-flex items-center gap-1.5"
            >
              <ExternalLink className="w-3 h-3" aria-hidden />
              Jira home
            </button>
          </li>
          <li className="text-[var(--text-tertiary)] inline-flex items-center gap-1.5">
            <Heart className="w-3 h-3" aria-hidden />
            Built with Tauri, React and a lot of coffee.
          </li>
        </ul>
      </section>
    </div>
  );
}
