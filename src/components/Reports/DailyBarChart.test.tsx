import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { WorklogRow } from "../../api/types";
import { DailyBarChart } from "./DailyBarChart";

function makeRow(dayOffset: number, durSec: number): WorklogRow {
  const d = new Date();
  d.setHours(10, 0, 0, 0);
  d.setDate(d.getDate() - dayOffset);
  return {
    issue_key: "ACME-1",
    duration_s: durSec,
    started_at: Math.floor(d.getTime() / 1000),
    logged_at: Math.floor(d.getTime() / 1000),
  };
}

describe("DailyBarChart", () => {
  it("renders an SVG with role img", () => {
    const today = new Date();
    const from = new Date(today);
    from.setDate(today.getDate() - 6);
    render(<DailyBarChart from={from} to={today} rows={[]} />);
    expect(
      screen.getByRole("img", { name: /daily worklog totals/i }),
    ).toBeInTheDocument();
  });

  it("renders bars for each day in the range and tooltips for totals", () => {
    const today = new Date();
    const from = new Date(today);
    from.setDate(today.getDate() - 3);
    const rows = [makeRow(1, 3600), makeRow(0, 1800)];
    const { container } = render(
      <DailyBarChart from={from} to={today} rows={rows} />,
    );
    // 4 days in the range → at least 4 bar rects + 5 gridline rects.
    const rects = container.querySelectorAll("rect");
    expect(rects.length).toBeGreaterThanOrEqual(4);
    // At least one tooltip mentions a worked total in human form.
    const titles = Array.from(container.querySelectorAll("title")).map(
      (t) => t.textContent,
    );
    expect(titles.some((t) => /1h/.test(t ?? ""))).toBe(true);
  });
});
