/**
 * Regression tests for useTauriEvent: the listener must subscribe ONCE per
 * event name regardless of how often the caller re-renders with a fresh inline
 * handler, and it must always dispatch to the latest handler. The old impl
 * listed `handler` in the effect deps, so an inline handler tore the listener
 * down and re-registered it (async) on every render — dropping events that
 * fired in the unsubscribed window.
 */
import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const captured: Array<(e: { payload: unknown }) => void> = [];
const unlisten = vi.fn();
const listen = vi.fn(
  async (_name: string, cb: (e: { payload: unknown }) => void) => {
    captured.push(cb);
    return unlisten;
  },
);

vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, cb: (e: { payload: unknown }) => void) =>
    listen(name, cb),
}));

import { useTauriEvent } from "./useTauriEvent";

describe("useTauriEvent", () => {
  beforeEach(() => {
    listen.mockClear();
    unlisten.mockClear();
    captured.length = 0;
  });

  it("subscribes once across re-renders with changing inline handlers", () => {
    const { rerender } = renderHook(({ h }) => useTauriEvent("evt", h), {
      initialProps: { h: vi.fn() },
    });
    rerender({ h: vi.fn() });
    rerender({ h: vi.fn() });
    expect(listen).toHaveBeenCalledTimes(1);
  });

  it("dispatches to the latest handler", () => {
    const h1 = vi.fn();
    const h2 = vi.fn();
    const { rerender } = renderHook(({ h }) => useTauriEvent("evt", h), {
      initialProps: { h: h1 },
    });
    rerender({ h: h2 });
    captured[0]?.({ payload: 42 });
    expect(h2).toHaveBeenCalledWith(42);
    expect(h1).not.toHaveBeenCalled();
  });
});
