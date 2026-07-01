/**
 * Shared "assign an issue to a worklog" handler.
 *
 * Both the Time Log rows and the Nepřiřazené screen let the user attach a task
 * to a worklog that was logged without one (timer stopped unassigned, or a
 * manual entry with no issue). The flow is identical: call the backend
 * `assign_worklog_issue`, refresh the worklog queries (which fans out to the
 * day view, history AND the unassigned badge via the `worklogs` prefix), and
 * toast the outcome. Centralised here so the two call sites can't drift.
 */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { assignWorklogIssue } from "../api/commands";
import { invalidateWorklogQueries } from "../api/queryKeys";
import type { WorklogRow } from "../api/types";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { useT } from "../i18n";

export type AssignWorklogFn = (
  row: WorklogRow,
  issueKey: string,
) => Promise<void>;

/**
 * Returns a stable handler that assigns `issueKey` to `row`. `pushToast` is the
 * shell's toast pusher (from `useOutletContext`); when omitted, outcomes are
 * silent.
 */
export function useAssignWorklog(
  pushToast?: ShellOutletContext["pushToast"],
): AssignWorklogFn {
  const t = useT();
  const queryClient = useQueryClient();
  return useCallback(
    async (row: WorklogRow, issueKey: string) => {
      if (row.id == null) return;
      try {
        await assignWorklogIssue(row.id, issueKey);
        invalidateWorklogQueries(queryClient);
        pushToast?.("success", t("worklog.assign.success", { issueKey }));
      } catch (e) {
        pushToast?.(
          "error",
          typeof e === "string" ? e : t("worklog.assign.error"),
        );
      }
    },
    [pushToast, queryClient, t],
  );
}
