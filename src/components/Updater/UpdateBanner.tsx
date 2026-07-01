/**
 * Top-of-window banner driven by `useUpdaterStore`.
 *
 * Renders nothing while idle/checking (the startup/daily check is silent).
 * When an update is available it offers Download → Restart, honoring the
 * time-tracker rule that we never restart silently and warn while a timer is
 * running (the timer resumes after relaunch, but the interruption is the
 * user's call, not ours).
 */
import { useUpdaterStore } from "../../stores/updaterStore";
import { useTimerStore } from "../../stores/timerStore";
import { useT } from "../../i18n";

function pct(downloaded: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.round((downloaded / total) * 100));
}

export function UpdateBanner() {
  const t = useT();
  const status = useUpdaterStore((s) => s.status);
  const version = useUpdaterStore((s) => s.version);
  const downloaded = useUpdaterStore((s) => s.downloaded);
  const total = useUpdaterStore((s) => s.total);
  const error = useUpdaterStore((s) => s.error);
  const download = useUpdaterStore((s) => s.download);
  const relaunch = useUpdaterStore((s) => s.relaunch);
  const dismiss = useUpdaterStore((s) => s.dismiss);
  const timerActive = useTimerStore((s) => s.active != null);

  // Silent states — nothing to show.
  if (status === "idle" || status === "checking") return null;

  const wrap = (children: React.ReactNode, tone: "info" | "danger" = "info") => (
    <div
      role="status"
      className="flex items-center gap-3 px-6 py-2 text-xs border-b"
      style={{
        background: "var(--bg-surface)",
        borderColor: "var(--border-subtle)",
        color: "var(--text-secondary)",
      }}
    >
      <span
        className="inline-block w-1.5 h-1.5 rounded-full shrink-0"
        style={{
          background: tone === "danger" ? "var(--danger)" : "var(--accent)",
        }}
        aria-hidden
      />
      {children}
    </div>
  );

  const btn = (
    label: string,
    onClick: () => void,
    primary = false,
  ) => (
    <button
      type="button"
      onClick={onClick}
      className="h-6 px-2 rounded-[var(--radius-sm)] text-[11px] transition-colors duration-150"
      style={
        primary
          ? { background: "var(--accent)", color: "var(--bg-app)" }
          : {
              border: "1px solid var(--border-subtle)",
              color: "var(--text-secondary)",
            }
      }
    >
      {label}
    </button>
  );

  if (status === "available") {
    return wrap(
      <>
        <span className="flex-1">
          {t("layout.updateAvailable", { version: version ? ` ${version}` : "" })}
        </span>
        {btn(t("layout.download"), () => void download(), true)}
        {btn(t("layout.later"), dismiss)}
      </>,
    );
  }

  if (status === "downloading") {
    const p = pct(downloaded, total);
    return wrap(
      <span className="flex-1">
        {t("layout.downloadingUpdate", {
          version: version ? ` ${version}` : "",
          pct: p != null ? ` ${p} %` : "",
        })}
      </span>,
    );
  }

  if (status === "ready") {
    return wrap(
      <>
        <span className="flex-1">
          {t("layout.updateReady", {
            version: version ? ` ${version}` : "",
            timerNote: timerActive ? t("layout.updateTimerNote") : "",
          })}
        </span>
        {btn(t("layout.restartAndFinish"), () => void relaunch(), !timerActive)}
        {btn(t("layout.later"), dismiss)}
      </>,
    );
  }

  // error
  return wrap(
    <>
      <span className="flex-1">
        {t("layout.updateFailed", { error: error ? `: ${error}` : "." })}
      </span>
      {btn(t("layout.close"), dismiss)}
    </>,
    "danger",
  );
}
