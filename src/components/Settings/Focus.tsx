/**
 * Settings → Focus.
 *
 *   Relace       [ Spustit Focus ]   Výchozí délka [ 50 min ▾ ]
 *   Aplikace     ☐ Povolit jen vybrané   + seznam pravidel
 *   Weby         ☐ Povolit jen vybrané   + seznam pravidel
 *   Notifikace   ☐ Ztlumit   + výběr zkratek (macOS) / odkaz do nastavení (Win)
 *
 * Rules are stored per `(kind, mode, pattern)`, so re-adding an existing entry
 * updates it rather than duplicating. Every mutating call returns the fresh
 * list, which is why there is no separate refetch after each edit.
 */
import { clsx } from "clsx";
import { Plus, ShieldBan, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import {
  addFocusRule,
  deleteFocusRule,
  getExtensionLastHeartbeat,
  getFocusSettings,
  listFocusRules,
  listFocusShortcuts,
  listRunningApps,
  openDndSettings,
  setFocusRuleAction,
  setFocusRuleEnabled,
  setFocusSettings,
} from "../../api/commands";
import type {
  FocusRule,
  FocusRuleAction,
  FocusRuleKind,
  FocusRuleMode,
  FocusSettings,
  RunningApp,
} from "../../api/types";
import { useFocusSession } from "../../hooks/useFocusSession";
import { useT } from "../../i18n";
import { formatRemaining } from "../../lib/focus";
import { Button } from "../common/Button";
import { Select } from "../common/Select";
import { SettingsCard } from "./SettingsCard";

/** Options for the default session length, in minutes. `0` = open-ended. */
const DURATIONS = [0, 25, 45, 50, 60, 90, 120];

/** The extension is considered connected if it called us in the last 2 min. */
const HEARTBEAT_FRESH_SECONDS = 120;

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

export default function Focus() {
  const t = useT();
  const session = useFocusSession();

  const [settings, setSettings] = useState<FocusSettings | null>(null);
  const [rules, setRules] = useState<FocusRule[]>([]);
  const [shortcuts, setShortcuts] = useState<string[]>([]);
  const [extensionFresh, setExtensionFresh] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getFocusSettings().then(setSettings).catch(() => setSettings(null));
    listFocusRules().then(setRules).catch(() => setRules([]));
    getExtensionLastHeartbeat()
      .then((seen) =>
        setExtensionFresh(
          seen != null && Date.now() / 1000 - seen < HEARTBEAT_FRESH_SECONDS,
        ),
      )
      .catch(() => setExtensionFresh(null));
  }, []);

  const reloadShortcuts = useCallback(() => {
    listFocusShortcuts().then(setShortcuts).catch(() => setShortcuts([]));
  }, []);
  useEffect(() => {
    if (isMac) reloadShortcuts();
  }, [reloadShortcuts]);

  const patch = useCallback(
    async (changes: Partial<FocusSettings>) => {
      if (!settings) return;
      const next = { ...settings, ...changes };
      setSettings(next);
      try {
        setSettings(await setFocusSettings(next));
        setError(null);
      } catch (e) {
        setError(errMessage(e));
        // Roll back to what the backend actually has.
        getFocusSettings().then(setSettings).catch(() => {});
      }
    },
    [settings],
  );

  const runRuleOp = useCallback(async (op: () => Promise<FocusRule[]>) => {
    try {
      setRules(await op());
      setError(null);
    } catch (e) {
      setError(errMessage(e));
    }
  }, []);

  if (!settings) {
    return (
      <div className="space-y-4">
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">{t("focus.title")}</h2>
        <p className="text-xs text-[var(--text-tertiary)]">{t("focus.settings.intro")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">{t("focus.title")}</h2>
        <p className="text-[12px] leading-relaxed text-[var(--text-tertiary)] mt-1">
          {t("focus.settings.intro")}
        </p>
      </div>

      {error && (
        <p className="text-xs" role="alert" style={{ color: "var(--danger)" }}>
          {error}
        </p>
      )}

      {/* Session ------------------------------------------------------- */}
      <SettingsCard title={t("focus.settings.sessionTitle")}>
        <div className="flex items-center justify-between gap-4 flex-wrap">
          <div className="flex items-center gap-3">
            <Button
              variant={session.active ? "danger" : "primary"}
              size="lg"
              onClick={() => void session.toggle()}
              disabled={session.busy}
            >
              {session.active ? t("focus.stop") : t("focus.start")}
            </Button>
            <span className="text-xs text-[var(--text-secondary)]">
              {session.active
                ? session.remainingSeconds != null
                  ? t("focus.remaining", { time: formatRemaining(session.remainingSeconds) })
                  : t("focus.openEnded")
                : t("focus.idle")}
            </span>
          </div>
          <label className="flex items-center gap-2 text-xs text-[var(--text-secondary)]">
            {t("focus.settings.durationLabel")}
            <Select
              value={String(settings.default_duration_min)}
              onChange={(e) => void patch({ default_duration_min: Number(e.target.value) })}
              options={DURATIONS.map((m) => ({
                value: String(m),
                label:
                  m === 0
                    ? t("focus.settings.durationOpen")
                    : t("focus.settings.durationMinutes", { count: m }),
              }))}
            />
          </label>
        </div>
      </SettingsCard>

      {/* Applications -------------------------------------------------- */}
      <SettingsCard
        title={t("focus.settings.appsTitle")}
        description={t("focus.settings.appsIntro")}
      >
        <SwitchRow
          label={t("focus.settings.strictApps")}
          hint={t("focus.settings.strictAppsHint")}
          checked={settings.strict_apps}
          onChange={(v) => void patch({ strict_apps: v })}
        />
        <div className="mt-4 grid gap-4 md:grid-cols-2">
          <RuleColumn
            kind="app"
            mode="block"
            rules={rules}
            onRun={runRuleOp}
            title={t("focus.rules.block")}
          />
          <RuleColumn
            kind="app"
            mode="allow"
            rules={rules}
            onRun={runRuleOp}
            title={t("focus.rules.allow")}
          />
        </div>
      </SettingsCard>

      {/* Websites ------------------------------------------------------ */}
      <SettingsCard
        title={t("focus.settings.sitesTitle")}
        description={t("focus.settings.sitesIntro")}
      >
        <SwitchRow
          label={t("focus.settings.strictSites")}
          hint={t("focus.settings.strictSitesHint")}
          checked={settings.strict_sites}
          onChange={(v) => void patch({ strict_sites: v })}
        />
        {extensionFresh !== null && (
          <p
            className="mt-3 text-[12px] leading-relaxed"
            style={{ color: extensionFresh ? "var(--text-tertiary)" : "var(--warning, var(--text-tertiary))" }}
          >
            {extensionFresh
              ? t("focus.settings.extensionOk")
              : t("focus.settings.extensionMissing")}
          </p>
        )}
        <div className="mt-4 grid gap-4 md:grid-cols-2">
          <RuleColumn
            kind="site"
            mode="block"
            rules={rules}
            onRun={runRuleOp}
            title={t("focus.rules.block")}
          />
          <RuleColumn
            kind="site"
            mode="allow"
            rules={rules}
            onRun={runRuleOp}
            title={t("focus.rules.allow")}
          />
        </div>
      </SettingsCard>

      {/* Notifications ------------------------------------------------- */}
      <SettingsCard title={t("focus.settings.notificationsTitle")}>
        <SwitchRow
          label={t("focus.settings.blockNotifications")}
          checked={settings.block_notifications}
          onChange={(v) => void patch({ block_notifications: v })}
        />
        {isMac ? (
          <div className="mt-4 space-y-3">
            <p className="text-[12px] leading-relaxed text-[var(--text-tertiary)]">
              {t("focus.settings.shortcutsIntro")}
            </p>
            <ShortcutPicker
              label={t("focus.settings.shortcutOn")}
              value={settings.shortcut_on}
              options={shortcuts}
              noneLabel={t("focus.settings.shortcutNone")}
              onChange={(v) => void patch({ shortcut_on: v })}
            />
            <ShortcutPicker
              label={t("focus.settings.shortcutOff")}
              value={settings.shortcut_off}
              options={shortcuts}
              noneLabel={t("focus.settings.shortcutNone")}
              onChange={(v) => void patch({ shortcut_off: v })}
            />
            <Button variant="secondary" size="sm" onClick={reloadShortcuts}>
              {t("focus.settings.shortcutReload")}
            </Button>
          </div>
        ) : (
          <div className="mt-4 space-y-3">
            <p className="text-[12px] leading-relaxed text-[var(--text-tertiary)]">
              {t("focus.settings.windowsDnd")}
            </p>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                openDndSettings().catch((e) => setError(errMessage(e)));
              }}
            >
              {t("focus.settings.openDnd")}
            </Button>
          </div>
        )}
      </SettingsCard>
    </div>
  );
}

// -----------------------------------------------------------------------------

/** One block/allow list for a given rule kind, with its own add form. */
function RuleColumn({
  kind,
  mode,
  rules,
  onRun,
  title,
}: {
  kind: FocusRuleKind;
  mode: FocusRuleMode;
  rules: FocusRule[];
  onRun: (op: () => Promise<FocusRule[]>) => Promise<void>;
  title: string;
}) {
  const t = useT();
  const [draft, setDraft] = useState("");
  const [apps, setApps] = useState<RunningApp[] | null>(null);
  const visible = rules.filter((r) => r.kind === kind && r.mode === mode);

  const submit = async () => {
    const pattern = draft.trim();
    if (!pattern) return;
    await onRun(() => addFocusRule({ kind, mode, pattern }));
    setDraft("");
  };

  return (
    <div>
      <h4 className="text-[11px] uppercase tracking-[0.06em] text-[var(--text-tertiary)] mb-2">
        {title}
      </h4>

      <ul className="space-y-1 mb-2">
        {visible.length === 0 && (
          <li className="text-xs text-[var(--text-tertiary)]">{t("focus.rules.empty")}</li>
        )}
        {visible.map((rule) => (
          <li
            key={rule.id}
            className="flex items-center gap-2 px-2 py-1.5 rounded-[var(--radius-md)]
                       bg-[var(--bg-app)] border border-[var(--border-subtle)]"
          >
            <input
              type="checkbox"
              checked={rule.enabled}
              aria-label={t("focus.rules.enabled")}
              onChange={(e) =>
                void onRun(() => setFocusRuleEnabled(rule.id, e.target.checked))
              }
              className="accent-[var(--accent)]"
            />
            <span
              className={clsx(
                "flex-1 min-w-0 truncate text-xs",
                rule.enabled ? "text-[var(--text-primary)]" : "text-[var(--text-tertiary)]",
              )}
              title={rule.pattern}
            >
              {rule.label ?? rule.pattern}
            </span>
            {kind === "app" && mode === "block" && (
              <Select
                value={rule.action}
                title={t("focus.rules.actionHint")}
                onChange={(e) =>
                  void onRun(() =>
                    setFocusRuleAction(rule.id, e.target.value as FocusRuleAction),
                  )
                }
                options={[
                  { value: "hide", label: t("focus.rules.actionHide") },
                  { value: "kill", label: t("focus.rules.actionKill") },
                ]}
              />
            )}
            <button
              type="button"
              aria-label={t("focus.rules.remove")}
              title={t("focus.rules.remove")}
              onClick={() => void onRun(() => deleteFocusRule(rule.id))}
              className="shrink-0 p-1 rounded hover:bg-[var(--bg-hover)] text-[var(--text-tertiary)]"
            >
              <Trash2 className="w-3.5 h-3.5" aria-hidden />
            </button>
          </li>
        ))}
      </ul>

      <div className="flex items-center gap-1.5">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void submit();
          }}
          placeholder={
            kind === "app" ? t("focus.settings.appPlaceholder") : t("focus.settings.sitePlaceholder")
          }
          className="flex-1 min-w-0 h-8 px-2 rounded-[var(--radius-md)] text-xs
                     bg-transparent border border-[var(--border-default)] text-[var(--text-primary)]
                     focus:outline-none focus:border-[var(--accent)]"
        />
        <Button size="sm" variant="secondary" onClick={() => void submit()} aria-label={t("focus.rules.add")}>
          <Plus className="w-3.5 h-3.5" aria-hidden />
        </Button>
      </div>

      {kind === "app" && (
        <div className="mt-2">
          <button
            type="button"
            className="text-[11px] underline text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
            onClick={() => {
              listRunningApps().then(setApps).catch(() => setApps([]));
            }}
          >
            {t("focus.settings.pickApp")}
          </button>
          {apps && (
            <ul className="mt-1.5 max-h-40 overflow-y-auto space-y-0.5">
              {apps.map((app) => (
                <li key={`${app.pattern}-${app.pid}`}>
                  <button
                    type="button"
                    disabled={app.protected}
                    onClick={() => {
                      void onRun(() =>
                        addFocusRule({ kind, mode, pattern: app.pattern, label: app.name }),
                      );
                    }}
                    className="w-full text-left px-2 py-1 rounded text-xs
                               hover:bg-[var(--bg-hover)] disabled:opacity-50
                               disabled:cursor-not-allowed text-[var(--text-secondary)]"
                  >
                    {app.name}
                    {app.protected && (
                      <span className="ml-1.5 text-[10px] text-[var(--text-tertiary)]">
                        ({t("focus.settings.protected")})
                      </span>
                    )}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

function SwitchRow({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="flex items-start gap-2.5 cursor-pointer">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 accent-[var(--accent)]"
      />
      <span className="min-w-0">
        <span className="block text-xs text-[var(--text-primary)]">{label}</span>
        {hint && (
          <span className="block text-[11px] leading-relaxed text-[var(--text-tertiary)] mt-0.5">
            {hint}
          </span>
        )}
      </span>
    </label>
  );
}

function ShortcutPicker({
  label,
  value,
  options,
  noneLabel,
  onChange,
}: {
  label: string;
  value: string | null;
  options: string[];
  noneLabel: string;
  onChange: (value: string | null) => void;
}) {
  // A previously-picked Shortcut that has since been renamed or deleted would
  // vanish from the list and silently reset the setting — keep it visible.
  const known = value && !options.includes(value) ? [value, ...options] : options;
  return (
    <label className="flex items-center justify-between gap-3 text-xs text-[var(--text-secondary)]">
      <span className="flex items-center gap-1.5">
        <ShieldBan className="w-3.5 h-3.5 text-[var(--text-tertiary)]" aria-hidden />
        {label}
      </span>
      <Select
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || null)}
        options={[
          { value: "", label: noneLabel },
          ...known.map((name) => ({ value: name, label: name })),
        ]}
      />
    </label>
  );
}

function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
