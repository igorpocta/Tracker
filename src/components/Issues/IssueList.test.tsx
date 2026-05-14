import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { IssueRow } from "../../api/types";
import { IssueList } from "./IssueList";

function issue(key: string, summary: string): IssueRow {
  return {
    issue_key: key,
    summary,
    updated_at: 0,
  };
}

describe("IssueList", () => {
  it("renders the title and rows", () => {
    render(
      <IssueList
        title="Recent"
        issues={[issue("ACME-1", "fix the bug"), issue("ACME-2", "ship it")]}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText("Recent")).toBeInTheDocument();
    expect(screen.getByText("ACME-1")).toBeInTheDocument();
    expect(screen.getByText("ACME-2")).toBeInTheDocument();
    expect(screen.getByText("fix the bug")).toBeInTheDocument();
  });

  it("shows the empty message when no issues are available", () => {
    render(<IssueList title="Recent" issues={[]} onSelect={vi.fn()} />);
    expect(screen.getByText(/nothing here yet/i)).toBeInTheDocument();
  });

  it("calls onSelect with the issue key when a row is clicked", async () => {
    const onSelect = vi.fn();
    render(
      <IssueList
        title="Recent"
        issues={[issue("ACME-1", "fix")]}
        onSelect={onSelect}
      />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /ACME-1.*fix/i }),
    );
    expect(onSelect).toHaveBeenCalledWith("ACME-1");
  });

  it("marks the selected row with aria-current", () => {
    render(
      <IssueList
        title="Recent"
        issues={[issue("ACME-1", "fix"), issue("ACME-2", "ship")]}
        selectedKey="ACME-1"
        onSelect={vi.fn()}
      />,
    );
    const selected = screen.getByRole("button", { name: /ACME-1.*fix/i });
    expect(selected).toHaveAttribute("aria-current", "true");
  });

  it("renders an active dot for the timer's issue", () => {
    render(
      <IssueList
        title="Recent"
        issues={[issue("ACME-1", "fix")]}
        activeKey="ACME-1"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByLabelText(/active timer/i)).toBeInTheDocument();
  });

  it("shows a loading message when loading with no issues yet", () => {
    render(
      <IssueList title="Recent" issues={[]} loading onSelect={vi.fn()} />,
    );
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });
});
