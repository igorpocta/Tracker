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
    <div className="flex flex-col gap-5 max-w-xl text-sm">
      <section>
        <h3 className="text-lg font-semibold">Tracker</h3>
        <p className="text-xs text-neutral-400 mt-1">
          A keyboard-friendly Jira time tracker.
        </p>
      </section>

      <dl className="grid grid-cols-3 gap-x-3 gap-y-2 text-xs">
        <dt className="text-neutral-500">Version</dt>
        <dd className="col-span-2 text-neutral-200 font-mono">0.1.0</dd>

        <dt className="text-neutral-500">Bundle id</dt>
        <dd className="col-span-2 text-neutral-200 font-mono">
          com.tracker.app
        </dd>

        <dt className="text-neutral-500">Platform</dt>
        <dd className="col-span-2 text-neutral-200">{platform}</dd>

        <dt className="text-neutral-500">User agent</dt>
        <dd className="col-span-2 text-neutral-400 break-words text-[11px]">
          {userAgent}
        </dd>
      </dl>

      <section className="border-t border-neutral-800/70 pt-4 flex flex-col gap-2">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-neutral-400">
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
              className="text-sky-400 hover:underline inline-flex items-center gap-1.5"
            >
              <ExternalLink className="w-3 h-3" aria-hidden />
              Jira home
            </button>
          </li>
          <li className="text-neutral-500 inline-flex items-center gap-1.5">
            <Heart className="w-3 h-3" aria-hidden />
            Built with Tauri, React and a lot of coffee.
          </li>
        </ul>
      </section>
    </div>
  );
}
