import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useTimerStore } from "../../stores/timerStore";
import { Timer } from "./Timer";

describe("Timer display", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useTimerStore.setState({ active: null, busy: false, error: null });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders --:--:-- when no timer is active", () => {
    render(<Timer />);
    expect(screen.getByText("--:--:--")).toBeInTheDocument();
  });

  it("renders the elapsed time when a timer is active", () => {
    vi.setSystemTime(new Date("2024-01-01T00:01:00Z"));
    useTimerStore.setState({
      active: {
        issue_key: "ACME-1",
        // 60s before "now"
        started_at: new Date("2024-01-01T00:00:00Z").getTime(),
        elapsed_seconds: 60,
      },
    });
    render(<Timer />);
    expect(screen.getByText("00:01:00")).toBeInTheDocument();
  });

  it("ticks once per second", () => {
    vi.setSystemTime(new Date("2024-01-01T00:00:00Z"));
    useTimerStore.setState({
      active: {
        issue_key: "ACME-1",
        started_at: new Date("2024-01-01T00:00:00Z").getTime(),
        elapsed_seconds: 0,
      },
    });
    render(<Timer />);
    expect(screen.getByText("00:00:00")).toBeInTheDocument();
    // Advance system time by 5s; the interval will fire ~5 times and
    // the display should pick up the latest Date.now().
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.getByText("00:00:05")).toBeInTheDocument();
  });
});
