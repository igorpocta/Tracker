/**
 * Unit tests for the `useTauriEvent` lifecycle guard.
 *
 * The hook's job is to bridge React's synchronous effect cleanup with
 * Tauri's `listen()` promise. The dangerous shape — and the one this
 * file pins against — is:
 *
 *   1. effect mounts → listen() returns a pending Promise<UnlistenFn>
 *   2. component unmounts BEFORE that promise resolves
 *   3. promise resolves later, hands us the unlisten function
 *   4. without the cancelled guard, that unlisten would never be
 *      called and the Tauri handler would leak.
 *
 * `useTauriEvent` carries a `let cancelled = false` flag and, in the
 * cleanup, sets it to true. When the promise finally resolves it
 * checks the flag and invokes the unlisten immediately if cleanup
 * already ran.
 */
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";
import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useTauriEvent } from "./useTauriEvent";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

const listenMock = vi.mocked(listen);

afterEach(() => {
  listenMock.mockReset();
});

describe("useTauriEvent", () => {
  it("invokes the unlisten returned by listen() on unmount", async () => {
    const unlistenSpy = vi.fn();
    listenMock.mockResolvedValue(unlistenSpy as unknown as UnlistenFn);
    const handler = vi.fn();

    const { unmount } = renderHook(() => useTauriEvent("happy-path", handler));
    // Let the listen() promise resolve before unmount so the hook
    // captures `unlisten` in its closure.
    await Promise.resolve();
    await Promise.resolve();

    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(unlistenSpy).not.toHaveBeenCalled();

    unmount();

    expect(unlistenSpy).toHaveBeenCalledTimes(1);
  });

  it("calls the late-resolved unlisten immediately when cleanup ran first", async () => {
    // Race we're pinning: component unmounts BEFORE listen() resolves.
    // Without the cancelled guard the resolved unlisten would never
    // be called and the Tauri handler would stay registered forever.
    let resolveListen!: (u: UnlistenFn) => void;
    const unlistenSpy = vi.fn();
    listenMock.mockImplementation(
      () =>
        new Promise<UnlistenFn>((res) => {
          resolveListen = res;
        }),
    );

    const { unmount } = renderHook(() => useTauriEvent("race", vi.fn()));

    // Unmount while listen() is still pending.
    unmount();
    expect(unlistenSpy).not.toHaveBeenCalled();

    // Now Tauri "finally" returns the unlisten — the hook's then()
    // handler must invoke it because `cancelled` is now true.
    resolveListen(unlistenSpy as unknown as UnlistenFn);
    await Promise.resolve(); // flush the then() microtask
    await Promise.resolve();

    expect(unlistenSpy).toHaveBeenCalledTimes(1);
  });

  it("survives a listen() rejection without crashing (e.g. non-Tauri test env)", async () => {
    listenMock.mockRejectedValue(new Error("not in a Tauri context"));

    const { unmount } = renderHook(() => useTauriEvent("rejected", vi.fn()));
    // Microtasks for the rejection.
    await Promise.resolve();
    await Promise.resolve();

    // No throw, cleanup is still safe to run.
    expect(() => unmount()).not.toThrow();
  });
});
