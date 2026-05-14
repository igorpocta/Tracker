/**
 * Vitest helpers for stubbing the Tauri IPC bridge.
 *
 * Tests typically don't import this file directly — instead they call
 * `vi.mock("@tauri-apps/api/core", ...)` at the top of the spec and use these
 * helpers to inspect the recorded calls.
 *
 * The shape mirrors the real `@tauri-apps/api/core` module just closely enough
 * to satisfy our typed wrappers in `src/api/commands.ts`.
 */
import { vi } from "vitest";

/** Single shared mock so tests can both arrange and assert on it. */
export const mockInvoke = vi.fn();

/** Module factory: pass to `vi.mock("@tauri-apps/api/core", () => coreMock)`. */
export const coreMock = {
  invoke: mockInvoke,
};

/** Module factory for the event module — tests rarely care about it. */
export const eventMock = {
  listen: vi.fn(async () => () => {
    /* noop unlisten */
  }),
  emit: vi.fn(async () => {}),
  emitTo: vi.fn(async () => {}),
};
