import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DailyGoalBar } from "./DailyGoalBar";

describe("DailyGoalBar", () => {
  it("renders logged + goal labels", () => {
    render(
      <DailyGoalBar loggedSeconds={1800} goalSeconds={8 * 60 * 60} />,
    );
    // 1800s = 30m
    expect(screen.getByText(/30m/)).toBeInTheDocument();
    // 8h goal
    expect(screen.getByText(/8h/)).toBeInTheDocument();
  });

  it("sets aria-valuenow to logged (clamped to max goal)", () => {
    render(
      <DailyGoalBar loggedSeconds={3600} goalSeconds={8 * 60 * 60} />,
    );
    const bar = screen.getByRole("progressbar", { name: /daily goal progress/i });
    expect(bar).toHaveAttribute("aria-valuenow", "3600");
    expect(bar).toHaveAttribute("aria-valuemax", `${8 * 60 * 60}`);
  });

  it("clamps the displayed value to the goal when over", () => {
    render(
      <DailyGoalBar loggedSeconds={10 * 3600} goalSeconds={8 * 60 * 60} />,
    );
    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", `${8 * 60 * 60}`);
  });

  it("handles zero goal without dividing by zero", () => {
    render(<DailyGoalBar loggedSeconds={0} goalSeconds={0} />);
    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuemax", "0");
  });
});
