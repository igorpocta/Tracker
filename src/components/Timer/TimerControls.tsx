/**
 * Stop dialog: opens when the user clicks "Stop", lets them adjust the
 * start time and optionally type a comment before committing the worklog.
 *
 * The dialog edits a *staged* start time and only writes it to the backend
 * (`update_timer_start`) at confirm-time, so backing out leaves the timer
 * intact.
 */
import { useEffect, useState } from "react";

import { useNow } from "../../hooks/useNow";
import { formatDuration } from "../../lib/format";
import type { ActiveTimerState } from "../../api/types";
import { Button } from "../common/Button";
import { Spinner } from "../common/Spinner";
import { CommentInput } from "./CommentInput";
import { StartTimeEditor } from "./StartTimeEditor";

export interface StopDialogProps {
  open: boolean;
  active: ActiveTimerState;
  busy?: boolean;
  /** Called with the final staged values; consumer applies them. */
  onConfirm: (params: {
    comment: string;
    startedAtMs: number | null;
  }) => Promise<void>;
  /**
   * Called when the user wants to throw the timer away without saving any
   * worklog (neither locally nor remotely). Consumer should invoke
   * `discardTimer()` from the api wrappers.
   */
  onDiscard?: () => Promise<void> | void;
  onClose: () => void;
}

export function StopDialog({
  open,
  active,
  busy = false,
  onConfirm,
  onDiscard,
  onClose,
}: StopDialogProps) {
  const [comment, setComment] = useState("");
  const [showStartEditor, setShowStartEditor] = useState(false);
  const [stagedStart, setStagedStart] = useState<number>(active.started_at);
  const now = useNow(1000);

  // Reset internal state whenever the dialog is (re)opened for a fresh timer.
  useEffect(() => {
    if (open) {
      setComment("");
      setShowStartEditor(false);
      setStagedStart(active.started_at);
    }
  }, [open, active.started_at]);

  if (!open) return null;

  const stagedElapsed = Math.max(0, Math.floor((now - stagedStart) / 1000));
  const startChanged = stagedStart !== active.started_at;

  const handleConfirm = async () => {
    await onConfirm({
      comment: comment.trim(),
      startedAtMs: startChanged ? stagedStart : null,
    });
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Zastavit časomíru"
      className="fixed inset-0 z-50 bg-[var(--bg-overlay)] backdrop-blur-sm flex items-center justify-center p-6"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !busy) onClose();
      }}
    >
      <div className="bg-[var(--bg-elevated)] border border-[var(--border-subtle)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] w-full max-w-md p-5 flex flex-col gap-4">
        <header>
          <h2 className="text-base font-semibold text-[var(--text-primary)]">Zastavit a uložit záznam</h2>
          <p className="text-xs text-[var(--text-secondary)] mt-0.5">
            <span className="font-mono">{active.issue_key}</span>
          </p>
        </header>

        <div className="flex items-baseline gap-2">
          <span className="text-3xl font-mono tabular-nums text-[var(--text-primary)]">
            {formatDuration(stagedElapsed)}
          </span>
          {startChanged && (
            <span className="text-[10px] uppercase tracking-wide text-[var(--warning)]">
              upraveno
            </span>
          )}
        </div>

        {showStartEditor ? (
          <StartTimeEditor
            startedAtMs={stagedStart}
            nowMs={now}
            onChange={setStagedStart}
          />
        ) : (
          <button
            type="button"
            onClick={() => setShowStartEditor(true)}
            className="self-start text-xs text-[var(--accent)] hover:underline underline-offset-2"
          >
            Upravit čas začátku
          </button>
        )}

        <CommentInput value={comment} onChange={setComment} disabled={busy} />

        <footer className="flex items-center justify-between gap-2 pt-1">
          {onDiscard ? (
            <button
              type="button"
              onClick={async () => {
                if (busy) return;
                if (
                  !window.confirm(
                    "Opravdu zahodit záznam? Časomíra bude zrušena a žádný worklog se neuloží.",
                  )
                ) {
                  return;
                }
                await onDiscard();
              }}
              disabled={busy}
              className="text-xs text-[var(--danger)] hover:underline underline-offset-2 disabled:opacity-50"
            >
              Zahodit záznam
            </button>
          ) : (
            <span />
          )}
          <div className="flex items-center gap-2">
            <Button variant="secondary" onClick={onClose} disabled={busy}>
              Zavřít
            </Button>
            <Button variant="primary" onClick={handleConfirm} disabled={busy}>
              {busy && <Spinner className="w-3.5 h-3.5" />}
              Zastavit a uložit
            </Button>
          </div>
        </footer>
      </div>
    </div>
  );
}
