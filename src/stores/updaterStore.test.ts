import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CHECK_INTERVAL_MS, useUpdaterStore } from "./updaterStore";
import * as updaterApi from "../api/updater";

vi.mock("../api/updater", () => ({
  checkForUpdate: vi.fn(),
  downloadAndInstall: vi.fn(),
  relaunchApp: vi.fn(),
}));

const checkForUpdate = vi.mocked(updaterApi.checkForUpdate);
const downloadAndInstall = vi.mocked(updaterApi.downloadAndInstall);

function fakeUpdate(version: string, body?: string) {
  // Only the fields the store reads; the real handle carries more.
  return { version, body } as unknown as updaterApi.Update;
}

function reset() {
  useUpdaterStore.setState({
    status: "idle",
    version: null,
    notes: null,
    downloaded: 0,
    total: null,
    error: null,
    lastCheckedAt: 0,
    _update: null,
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  reset();
});

afterEach(() => {
  reset();
});

describe("updaterStore.check", () => {
  it("goes to 'available' with version + notes when the feed has an update", async () => {
    checkForUpdate.mockResolvedValue(fakeUpdate("1.2.0", "changelog"));
    await useUpdaterStore.getState().check();
    const s = useUpdaterStore.getState();
    expect(s.status).toBe("available");
    expect(s.version).toBe("1.2.0");
    expect(s.notes).toBe("changelog");
  });

  it("stays idle when already up to date", async () => {
    checkForUpdate.mockResolvedValue(null);
    await useUpdaterStore.getState().check();
    expect(useUpdaterStore.getState().status).toBe("idle");
  });

  it("swallows errors on a silent check (no nag)", async () => {
    checkForUpdate.mockRejectedValue(new Error("network down"));
    await useUpdaterStore.getState().check({ silent: true });
    expect(useUpdaterStore.getState().status).toBe("idle");
  });

  it("surfaces errors on a manual check", async () => {
    checkForUpdate.mockRejectedValue(new Error("network down"));
    await useUpdaterStore.getState().check({ silent: false });
    const s = useUpdaterStore.getState();
    expect(s.status).toBe("error");
    expect(s.error).toContain("network down");
  });

  it("records lastCheckedAt so the daily throttle can gate future checks", async () => {
    checkForUpdate.mockResolvedValue(null);
    await useUpdaterStore.getState().check();
    expect(useUpdaterStore.getState().lastCheckedAt).toBeGreaterThan(0);
  });
});

describe("updaterStore.maybeCheckOnStartup", () => {
  it("skips the check when the last one was within the interval", async () => {
    const now = 1_000_000_000_000;
    useUpdaterStore.setState({ lastCheckedAt: now - 1000 });
    await useUpdaterStore.getState().maybeCheckOnStartup(now);
    expect(checkForUpdate).not.toHaveBeenCalled();
  });

  it("checks when the interval has elapsed", async () => {
    checkForUpdate.mockResolvedValue(null);
    const now = 1_000_000_000_000;
    useUpdaterStore.setState({ lastCheckedAt: now - CHECK_INTERVAL_MS - 1 });
    await useUpdaterStore.getState().maybeCheckOnStartup(now);
    expect(checkForUpdate).toHaveBeenCalledTimes(1);
  });
});

describe("updaterStore.download", () => {
  it("reaches 'ready' after a successful download+install", async () => {
    useUpdaterStore.setState({ _update: fakeUpdate("1.2.0"), status: "available" });
    downloadAndInstall.mockResolvedValue(undefined);
    await useUpdaterStore.getState().download();
    expect(useUpdaterStore.getState().status).toBe("ready");
  });

  it("goes to 'error' when the download fails", async () => {
    useUpdaterStore.setState({ _update: fakeUpdate("1.2.0"), status: "available" });
    downloadAndInstall.mockRejectedValue(new Error("bad signature"));
    await useUpdaterStore.getState().download();
    const s = useUpdaterStore.getState();
    expect(s.status).toBe("error");
    expect(s.error).toContain("bad signature");
  });

  it("is a no-op without a pending update", async () => {
    await useUpdaterStore.getState().download();
    expect(downloadAndInstall).not.toHaveBeenCalled();
  });
});
