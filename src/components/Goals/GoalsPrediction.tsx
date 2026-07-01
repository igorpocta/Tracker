/**
 * "Predikce konce měsíce" card for the Goals route — Phase 18B Item 3.
 *
 * Given the user's current pace (avg seconds per working day so far in the
 * month) and the number of working days remaining, projects the month-end
 * total and compares it to the monthly goal.
 *
 *  ┌─ Predikce konce měsíce ───────────────────────────────────────────┐
 *  │ Průměr letošního měsíce: 6 h 12 m / den                           │
 *  │ Zbývá: 10 pracovních dnů                                          │
 *  │ Predikce: 39 h 24 m + 62 h = 101 h 24 m (cíl 189 h)               │
 *  │ Tempo: budete 87 h pod cílem                                      │
 *  └────────────────────────────────────────────────────────────────────┘
 */
import { useT } from "../../i18n";
import { formatDurationShort } from "../../lib/format";

export interface GoalsPredictionProps {
  /** Seconds logged so far this month (all worklogs in current month). */
  actualSeconds: number;
  /** Monthly goal in seconds (working_days_in_month × daily_goal). */
  monthlyGoalSeconds: number;
  /** Working days that have elapsed (from month start through today inclusive). */
  workingDaysElapsed: number;
  /** Working days remaining (from tomorrow through month end). */
  workingDaysRemaining: number;
}

export function GoalsPrediction({
  actualSeconds,
  monthlyGoalSeconds,
  workingDaysElapsed,
  workingDaysRemaining,
}: GoalsPredictionProps) {
  const t = useT();
  const avgPerWorkingDay =
    workingDaysElapsed > 0 ? actualSeconds / workingDaysElapsed : 0;
  const predictedAdditional = Math.round(avgPerWorkingDay * workingDaysRemaining);
  const predictedTotal = actualSeconds + predictedAdditional;
  const diffVsGoal = predictedTotal - monthlyGoalSeconds;

  const diffTone: "danger" | "success" | "neutral" =
    diffVsGoal < 0 ? "danger" : diffVsGoal > 0 ? "success" : "neutral";

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
        {t("misc.prediction.heading")}
      </h3>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-2 text-xs">
        <Row
          label={t("misc.prediction.avgLabel")}
          value={t("misc.prediction.avgValue", {
            duration: formatDurationShort(Math.round(avgPerWorkingDay)),
          })}
        />
        <Row
          label={t("misc.prediction.remainingLabel")}
          value={t("misc.prediction.remainingValue", {
            days: workingDaysRemaining,
          })}
        />
        <Row
          label={t("misc.prediction.predictionLabel")}
          value={
            <span className="font-mono tabular-nums">
              {formatDurationShort(actualSeconds)} +{" "}
              {formatDurationShort(predictedAdditional)} ={" "}
              <span className="text-[var(--accent)]">
                {formatDurationShort(predictedTotal)}
              </span>{" "}
              <span className="text-[var(--text-tertiary)]">
                {t("misc.prediction.goalSuffix", {
                  duration: formatDurationShort(monthlyGoalSeconds),
                })}
              </span>
            </span>
          }
        />
        <Row
          label={t("misc.prediction.paceLabel")}
          value={
            <span
              className="font-medium"
              style={{
                color:
                  diffTone === "danger"
                    ? "var(--danger)"
                    : diffTone === "success"
                      ? "var(--success)"
                      : "var(--text-primary)",
              }}
            >
              {diffVsGoal === 0
                ? t("misc.prediction.onTarget")
                : diffVsGoal < 0
                  ? t("misc.prediction.under", {
                      duration: formatDurationShort(Math.abs(diffVsGoal)),
                    })
                  : t("misc.prediction.over", {
                      duration: formatDurationShort(diffVsGoal),
                    })}
            </span>
          }
        />
      </div>
    </div>
  );
}

function Row({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <span className="text-[var(--text-tertiary)]">{label}:</span>
      <span className="text-[var(--text-primary)] text-right">{value}</span>
    </div>
  );
}
