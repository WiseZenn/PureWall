import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: class {
    onmessage = (_event: unknown) => undefined;
  },
}));

import { useAppUpdatesStore } from "./appUpdates";

const update = {
  version: "0.2.0",
  currentVersion: "0.1.0",
  date: "2026-08-01T00:00:00Z",
  body: "Bug fixes",
};

describe("application update state machine", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
  });

  it("moves from checking to current when no update is returned", async () => {
    invokeMock.mockResolvedValueOnce(null);
    const store = useAppUpdatesStore();

    const check = store.checkForUpdates();
    expect(store.state.status).toBe("checking");
    await check;

    expect(store.state).toEqual({ status: "current", currentVersion: "0.1.0" });
  });

  it("moves from checking to available and installs with progress", async () => {
    invokeMock.mockResolvedValueOnce(update);
    const store = useAppUpdatesStore();
    await store.checkForUpdates();
    expect(store.state).toEqual({ status: "available", update });

    invokeMock.mockImplementationOnce(async (_command: string, args: { onEvent: { onmessage: (event: unknown) => void } }) => {
      args.onEvent.onmessage({ event: "started", data: { contentLength: 100 } });
      args.onEvent.onmessage({ event: "progress", data: { downloaded: 40, contentLength: 100 } });
    });
    const install = store.installUpdate();
    expect(store.state.status).toBe("downloading");
    await install;

    expect(store.state).toEqual({ status: "readyToRestart", update });
    expect(invokeMock).toHaveBeenLastCalledWith("install_update", expect.any(Object));
  });

  it("ignores concurrent checks and installs, and exposes retry on errors", async () => {
    let resolveCheck!: (value: null) => void;
    invokeMock.mockReturnValueOnce(new Promise((resolve) => { resolveCheck = resolve; }));
    const store = useAppUpdatesStore();
    const first = store.checkForUpdates();
    const second = store.checkForUpdates();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(store.installUpdate()).resolves.toBeUndefined();
    resolveCheck(null);
    await Promise.all([first, second]);

    invokeMock.mockRejectedValueOnce(new Error("offline"));
    await store.checkForUpdates();
    expect(store.state).toMatchObject({ status: "error", retry: "check" });
  });
});
