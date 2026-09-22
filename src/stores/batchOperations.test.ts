import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { currentWebviewWindowMock, invokeMock, listenMock } = vi.hoisted(() => ({
  currentWebviewWindowMock: vi.fn(),
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: currentWebviewWindowMock,
}));

import { useWallpaperStore } from "./wallpapers";

const emptyPage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 96,
  has_more: false,
};

const emptyStats = {
  total: 0,
  liked: 0,
  disliked: 0,
  blacklisted: 0,
  total_plays: 0,
};

function selectTwo(store: ReturnType<typeof useWallpaperStore>) {
  store.selectionMode = true;
  store.selectedPaths.add("C:\\Walls\\first.jpg");
  store.selectedPaths.add("C:\\Walls\\second.jpg");
}

describe("batch wallpaper operations", () => {
  beforeEach(() => {
    vi.stubGlobal("window", {
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
    });
    setActivePinia(createPinia());
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => undefined);
    currentWebviewWindowMock.mockReset();
    currentWebviewWindowMock.mockReturnValue({ label: "main" });
  });

  it("clears selection and follows refresh hints after a successful tag removal", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "batch_unassign_tag") {
        return { affected: 2, refresh: ["wallpapers"] };
      }
      if (command === "get_wallpapers_page") return emptyPage;
      return undefined;
    });
    const store = useWallpaperStore();
    selectTwo(store);

    const succeeded = await store.batchUnassignTag(7);

    expect(succeeded).toBe(true);
    expect(store.selectedCount).toBe(0);
    expect(store.selectionMode).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("batch_unassign_tag", {
      paths: ["C:\\Walls\\first.jpg", "C:\\Walls\\second.jpg"],
      tagId: 7,
    });
    expect(invokeMock).toHaveBeenCalledWith("get_wallpapers_page", {
      filter: "all",
      sort: "created",
      search: "",
      offset: 0,
      limit: 96,
    });
  });

  it("returns false and preserves selection when a mutation fails", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "operation_failed",
      message: "Database is busy.",
    });
    const store = useWallpaperStore();
    selectTwo(store);

    const succeeded = await store.batchSetRating(1);

    expect(succeeded).toBe(false);
    expect(store.selectedCount).toBe(2);
    expect(store.selectionMode).toBe(true);
  });

  it("routes collection removal and refreshes wallpapers and collection counts", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "batch_unassign_collection") {
        return {
          affected: 2,
          refresh: ["wallpapers", "collections"],
        };
      }
      if (command === "get_wallpapers_page") return emptyPage;
      if (command === "get_collections") return [];
      return undefined;
    });
    const store = useWallpaperStore();
    selectTwo(store);

    await expect(store.batchUnassignCollection(11)).resolves.toBe(true);

    expect(invokeMock).toHaveBeenCalledWith("batch_unassign_collection", {
      paths: ["C:\\Walls\\first.jpg", "C:\\Walls\\second.jpg"],
      collectionId: 11,
    });
    expect(invokeMock).toHaveBeenCalledWith("get_collections");
  });

  it("blocks a repeated mutation while the first command is pending", async () => {
    let resolveCommand: ((value: unknown) => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      if (command === "batch_set_rating") {
        return new Promise((resolve) => {
          resolveCommand = resolve;
        });
      }
      if (command === "get_stats") return Promise.resolve(emptyStats);
      return Promise.resolve(undefined);
    });
    const store = useWallpaperStore();
    selectTwo(store);

    const first = store.batchSetRating(1);
    await expect(store.batchSetRating(-1)).resolves.toBe(false);
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "batch_set_rating"),
    ).toHaveLength(1);

    resolveCommand?.({ affected: 2, refresh: ["stats"] });
    await expect(first).resolves.toBe(true);
    expect(store.isBatchMutating).toBe(false);
  });
});
