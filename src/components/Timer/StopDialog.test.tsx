/**
 * Regression: the "Zastavit a uložit" button was disabled only by the async
 * `busy` prop, which flips true a tick after the store action starts. A fast
 * double-click fired onConfirm twice before busy propagated — two stops, a
 * duplicate worklog. A synchronous in-flight guard must collapse repeats.
 */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ActiveTimerState } from "../../api/types";
import { StopDialog } from "./TimerControls";

const active: ActiveTimerState = {
  issue_key: "DEV-1",
  started_at: 1_000_000,
  elapsed_seconds: 60,
};

describe("StopDialog confirm guard", () => {
  it("fires onConfirm once even on a rapid double-click", async () => {
    let resolve: (() => void) | undefined;
    const onConfirm = vi.fn(
      () =>
        new Promise<void>((r) => {
          resolve = () => r();
        }),
    );
    const user = userEvent.setup();
    render(
      <StopDialog
        open
        active={active}
        onConfirm={onConfirm}
        onClose={vi.fn()}
      />,
    );

    const btn = screen.getByRole("button", { name: /Zastavit a uložit/ });
    await user.click(btn);
    await user.click(btn);

    expect(onConfirm).toHaveBeenCalledTimes(1);
    resolve?.();
  });
});
