import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { WorklogRow } from "../../api/types";
import { ProjectDonutChart } from "./ProjectDonutChart";

function makeRow(key: string, durSec: number): WorklogRow {
  return {
    issue_key: key,
    duration_s: durSec,
    started_at: Math.floor(Date.now() / 1000),
    logged_at: Math.floor(Date.now() / 1000),
  };
}

describe("ProjectDonutChart", () => {
  it("renders an empty hint when no rows", () => {
    render(<ProjectDonutChart rows={[]} />);
    expect(screen.getByTestId("project-donut-empty")).toBeInTheDocument();
  });

  it("renders one legend row per project prefix", () => {
    const rows: WorklogRow[] = [
      makeRow("ACME-1", 3600),
      makeRow("ACME-2", 1800),
      makeRow("PROJ-1", 7200),
    ];
    render(<ProjectDonutChart rows={rows} />);
    expect(screen.getByTestId("project-donut-chart")).toBeInTheDocument();
    expect(screen.getByText("ACME")).toBeInTheDocument();
    expect(screen.getByText("PROJ")).toBeInTheDocument();
  });

  it("buckets extra projects into 'Other'", () => {
    const rows: WorklogRow[] = Array.from({ length: 12 }, (_, i) =>
      makeRow(`P${i}-1`, 60 * (i + 1)),
    );
    render(<ProjectDonutChart rows={rows} maxSlices={3} />);
    expect(screen.getByText("Other")).toBeInTheDocument();
  });

  it("displays a total in the donut center", () => {
    const rows = [makeRow("X-1", 3600), makeRow("Y-1", 1800)];
    const { container } = render(<ProjectDonutChart rows={rows} />);
    // Total = 1h30m -> formatDurationShort -> "1h 30m"
    expect(container.textContent).toMatch(/1h\s30m/);
  });
});
