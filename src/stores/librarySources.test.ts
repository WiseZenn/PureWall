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

const emptyStats = {
  total: 0,
  liked: 0,
  disliked: 0,
  blacklisted: 0,
  total_plays: 0,
};

describe("library source store routing", () => {
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

  it("loads and stores exact source DTOs", async () => {
    invokeMock.mockResolvedValueOnce([
      {
        path: "D:\\Walls",
        source: "mounted",
        status: "online",
        available_count: 12,
        unavailable_count: 2,
        last_scan_at: "2026-07-28 10:00:00",
        last_error: null,
      },
    ]);
    const store = useWallpaperStore();

    await store.loadLibrarySources();

    expect(invokeMock).toHaveBeenCalledWith("list_library_sources");
    expect(store.librarySources).toHaveLength(1);
    expect(store.librarySources[0].available_count).toBe(12);
  });

  it("routes remove preview and keep-metadata confirmation exactly", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "preview_remove_library_source") {
        return { affected_wallpapers: 4 };
      }
      if (command === "remove_library_source") {
        return {
          affected_wallpapers: 4,
          matched_wallpapers: 0,
          imported_wallpapers: 0,
          unavailable_wallpapers: 4,
          watcher_warning: null,
        };
      }
      if (command === "list_library_sources") return [];
      if (command === "get_wallpapers_page") {
        return {
          items: [],
          total: 0,
          offset: 0,
          limit: 96,
          has_more: false,
        };
      }
      if (command === "get_stats") return emptyStats;
      return undefined;
    });
    const store = useWallpaperStore();

    await expect(
      store.previewRemoveLibrarySource("D:\\Walls"),
    ).resolves.toEqual({
      affected_wallpapers: 4,
    });
    await store.removeLibrarySource("D:\\Walls", "keep_metadata");

    expect(invokeMock).toHaveBeenCalledWith("remove_library_source", {
      path: "D:\\Walls",
      mode: "keep_metadata",
    });
    expect(store.librarySourceBusy.has("D:\\Walls")).toBe(false);
  });

  it("keeps the row error and clears busy state when relocate fails", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "SOURCE_RELOCATE_COLLISION",
      message: "Target wallpaper path is already owned.",
    });
    const store = useWallpaperStore();

    await expect(
      store.relocateLibrarySource("D:\\Walls", "E:\\Walls"),
    ).rejects.toMatchObject({ code: "SOURCE_RELOCATE_COLLISION" });
    expect(store.librarySourceBusy.has("D:\\Walls")).toBe(false);
    expect(store.librarySourceErrors.get("D:\\Walls")).toContain(
      "Target wallpaper path is already owned.",
    );
  });
});
