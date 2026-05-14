/**
 * Settings → Goals.
 *
 * Reference: `screens/SCR-20260514-rjjq-2.png`.
 *
 *   Daily hours goal                                                9h
 *   1h ────────●────────────────── 14h
 *
 *   How many hours you aim to work each working day. Used in the Goals view.
 */
import { usePrefsStore } from "../../stores/prefsStore";

const MIN_HOURS = 1;
const MAX_HOURS = 14;

export default function SettingsGoals() {
  const goalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const setGoal = usePrefsStore((s) => s.setDailyGoal);

  const hours = Math.max(MIN_HOURS, Math.min(MAX_HOURS, goalSeconds / 3600));

  return (
    <div className="flex flex-col gap-6 max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Cíle
        </h2>
      </header>

      <section>
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-semibold text-[var(--text-primary)]">
            Denní cíl hodin
          </span>
          <span className="text-lg font-semibold text-[var(--accent)] tabular-nums">
            {hours}h
          </span>
        </div>
        <input
          type="range"
          min={MIN_HOURS}
          max={MAX_HOURS}
          step={0.5}
          value={hours}
          onChange={(e) => {
            const h = parseFloat(e.target.value);
            void setGoal(Math.round(h * 3600));
          }}
          className="w-full h-1.5 rounded-full appearance-none cursor-pointer
                     bg-[var(--accent-soft)] outline-none"
          style={{
            // Custom range thumb color via accent.
            // CSS variable in a string isn't easily themable via Tailwind here.
            accentColor: "var(--accent)",
          }}
        />
        <div className="flex items-center justify-between mt-1 text-[11px] text-[var(--text-tertiary)] tabular-nums">
          <span>{MIN_HOURS}h</span>
          <span>{MAX_HOURS}h</span>
        </div>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-3">
          Kolik hodin chcete denně odpracovat. Používá se v sekci Cíle.
        </p>
      </section>
    </div>
  );
}
