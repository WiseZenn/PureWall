import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import {
  createWidgetCommandController,
  getWidgetCommandErrorMessage,
} from "../views/WidgetView.vue";

const { currentWebviewWindowMock, invokeMock, listenMock, listenerHandlers } = vi.hoisted(() => ({
  currentWebviewWindowMock: vi.fn(),
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  listenerHandlers: new Map<string, (event: { payload: unknown }) => unknown>(),
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

const emptyYearlyStats = {
  year: 2026,
  total_plays: 0,
  unique_wallpapers: 0,
  liked_plays: 0,
  monthly: [],
  top_wallpapers: [],
};

function wallpaper(path: string, rating = 0) {
  return {
    id: 1,
    path,
    hash: "hash",
    source: "test",
    display_title: "",
    rating,
    play_count: 0,
    last_played: null,
    created_at: "2026-01-01T00:00:00Z",
    blacklisted: false,
    width: 1920,
    height: 1080,
    file_size: 42,
    tags: [],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((next, fail) => {
    resolve = next;
    reject = fail;
  });
  return { promise, resolve, reject };
}

async function flush() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("wallpaper Store command routing", () => {
  beforeEach(() => {
    vi.stubGlobal("window", {
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
    });
    setActivePinia(createPinia());
    invokeMock.mockReset();
    currentWebviewWindowMock.mockReset();
    currentWebviewWindowMock.mockReturnValue({ label: "main" });
    listenMock.mockReset();
    listenerHandlers.clear();
    listenMock.mockImplementation(async (eventName: string, handler: (event: { payload: unknown }) => unknown) => {
      listenerHandlers.set(eventName, handler);
      return () => listenerHandlers.delete(eventName);
    });
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_stats") return emptyStats;
      if (command === "get_yearly_stats") return emptyYearlyStats;
      if (command === "get_wallpaper_by_path") return wallpaper("D:/walls/current.jpg");
      if (command === "get_image_metadata") return { width: 1920, height: 1080, file_size: 42 };
      if (command === "get_shell_metadata") return { authors: "", copyright: "", comment: "" };
      if (command === "load_preview_image") return null;
      if (command === "create_collection") {
        return {
          id: 7,
          name: "Weekend",
          color: "#5b8def",
          wallpaper_count: 0,
        };
      }
      return undefined;
    });
  });

  it("routes collection creation to create_collection", async () => {
    const store = useWallpaperStore();

    await store.createCollection("Weekend", "#5b8def");

    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("create_collection", {
      name: "Weekend",
      color: "#5b8def",
    });
  });

  it("routes Like, Dislike, and restore to their exact rating commands", async () => {
    const store = useWallpaperStore();
    const path = "D:/walls/current.jpg";

    await store.like(path);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "like_wallpaper", { path });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_stats");

    invokeMock.mockClear();
    await store.dislike(path);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "dislike_wallpaper", { path });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_stats");

    invokeMock.mockClear();
    await store.resetRating(path);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "reset_rating", { path });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_stats");
  });

  it("routes current rating through one typed command without treating its target as a transition", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "run_playback_action") {
        return {
          action: "like",
          current_wallpaper_path: "D:/walls/current.jpg",
          rating: 1,
          paused: null,
        };
      }
      if (command === "get_stats") return emptyStats;
      if (command === "get_yearly_stats") return emptyYearlyStats;
      return undefined;
    });
    const store = useWallpaperStore();

    await store.likeCurrentWallpaper();

    expect(invokeMock).toHaveBeenCalledWith("run_playback_action", { action: "like" });
    expect(store.currentWallpaperPath).toBe("");
    expect(store.activeWallpaperPath).toBe("");
    expect(invokeMock).toHaveBeenCalledWith("get_stats");
    expect(invokeMock).toHaveBeenCalledWith("get_yearly_stats", { year: 2026 });
    expect(invokeMock).not.toHaveBeenCalledWith("load_preview_image", {
      path: "D:/walls/current.jpg",
    });
  });

  it("applies pause outcomes from the shared action command", async () => {
    invokeMock.mockResolvedValue({
      action: "toggle_pause",
      current_wallpaper_path: null,
      rating: null,
      paused: true,
    });
    const store = useWallpaperStore();

    await store.togglePause();

    expect(store.isPaused).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("run_playback_action", { action: "toggle_pause" });
  });

  it("routes every playback facade through the typed shared command", async () => {
    invokeMock.mockImplementation(async (command: string, args?: { action?: string }) => {
      if (command === "run_playback_action") {
        return {
          action: args?.action,
          current_wallpaper_path: args?.action === "toggle_pause" ? null : "D:/walls/current.jpg",
          rating: args?.action === "like" ? 1 : args?.action === "dislike" ? -1 : null,
          paused: args?.action === "toggle_pause" ? true : null,
        };
      }
      if (command === "get_stats") return emptyStats;
      if (command === "get_yearly_stats") return emptyYearlyStats;
      if (command === "get_wallpaper_by_path") return wallpaper("D:/walls/current.jpg");
      if (command === "get_image_metadata") return { width: 1920, height: 1080, file_size: 42 };
      if (command === "get_shell_metadata") return { authors: "", copyright: "", comment: "" };
      if (command === "load_preview_image") return null;
      return undefined;
    });
    const store = useWallpaperStore();

    await store.nextWallpaper();
    await store.likeCurrentWallpaper();
    await store.dislikeCurrentWallpaper();
    await store.togglePause();

    expect(invokeMock.mock.calls.filter(([command]) => command === "run_playback_action")).toEqual([
      ["run_playback_action", { action: "next" }],
      ["run_playback_action", { action: "like" }],
      ["run_playback_action", { action: "dislike" }],
      ["run_playback_action", { action: "toggle_pause" }],
    ]);
  });

  it("applies a Next transition with preview and metadata outside the gallery page", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "run_playback_action") {
        return {
          action: "next",
          current_wallpaper_path: "D:/walls/outside-page.jpg",
          rating: null,
          paused: null,
        };
      }
      if (command === "get_wallpaper_by_path") return wallpaper("D:/walls/outside-page.jpg");
      if (command === "get_image_metadata") return { width: 2560, height: 1440, file_size: 99 };
      if (command === "get_shell_metadata") return { authors: "Artist", copyright: "", comment: "" };
      if (command === "load_preview_image") return null;
      if (command === "get_stats") return emptyStats;
      if (command === "get_yearly_stats") return emptyYearlyStats;
      return undefined;
    });
    const store = useWallpaperStore();

    await store.runPlaybackAction("next");
    await flush();

    expect(store.currentWallpaperPath).toBe("D:/walls/outside-page.jpg");
    expect(store.activeWallpaperPath).toBe("D:/walls/outside-page.jpg");
    expect(store.currentWallpaper).toMatchObject({
      path: "D:/walls/outside-page.jpg",
      rating: 0,
      width: 2560,
      height: 1440,
    });
    expect(store.isPaused).toBe(false);
    expect(store.activeWallpaper).toMatchObject({ path: "D:/walls/outside-page.jpg", rating: 0 });
    expect(store.activeMetadata).toEqual({ width: 2560, height: 1440, file_size: 99 });
    expect(store.activeShellMetadata).toEqual({ authors: "Artist", copyright: "", comment: "" });
    expect(invokeMock).toHaveBeenCalledWith("load_preview_image", { path: "D:/walls/outside-page.jpg" });
    expect(invokeMock).toHaveBeenCalledWith("get_stats");
    expect(invokeMock).toHaveBeenCalledWith("get_yearly_stats", { year: 2026 });
  });

  it("keeps an external Next transition when an earlier local rating outcome completes later", async () => {
    const firstPath = "D:/walls/a.jpg";
    const nextPath = "D:/walls/b.jpg";
    const localOutcome = deferred<unknown>();
    invokeMock.mockImplementation((command: string, args?: { action?: string; path?: string }) => {
      if (command === "get_wallpapers_page") {
        return Promise.resolve({
          items: [wallpaper(firstPath), wallpaper(nextPath)],
          total: 2,
          offset: 0,
          limit: 60,
          has_more: false,
        });
      }
      if (command === "run_playback_action") return localOutcome.promise;
      if (command === "get_wallpaper_by_path") return Promise.resolve(wallpaper(args?.path ?? nextPath));
      if (command === "get_image_metadata") return Promise.resolve({ width: 1920, height: 1080, file_size: 42 });
      if (command === "get_shell_metadata") return Promise.resolve({ authors: "", copyright: "", comment: "" });
      if (command === "load_preview_image") return Promise.resolve(null);
      if (command === "get_stats") return Promise.resolve(emptyStats);
      if (command === "get_yearly_stats") return Promise.resolve(emptyYearlyStats);
      return Promise.resolve(undefined);
    });
    const store = useWallpaperStore();
    await store.loadWallpapers();
    await flush();
    invokeMock.mockClear();
    await store.setupListeners();

    const running = store.likeCurrentWallpaper();
    await listenerHandlers.get("auto-rotated")?.({ payload: nextPath });
    await flush();
    localOutcome.resolve({ action: "like", current_wallpaper_path: firstPath, rating: 1, paused: null });
    await running;

    for (const command of ["load_preview_image", "get_image_metadata", "get_shell_metadata", "get_wallpaper_by_path"]) {
      expect(invokeMock.mock.calls.filter(([called]) => called === command)).toHaveLength(1);
    }
    for (const command of ["get_stats", "get_yearly_stats"]) {
      expect(invokeMock.mock.calls.filter(([called]) => called === command)).toHaveLength(2);
    }
    expect(store.currentWallpaperPath).toBe(nextPath);
    expect(store.activeWallpaperPath).toBe(nextPath);
    expect(store.wallpapers.find((entry) => entry.path === firstPath)?.rating).toBe(1);
  });

  it("serializes local rating commands, refreshes the current target, and preserves order", async () => {
    const currentPath = "D:/walls/current.jpg";
    let currentRating = 0;
    const like = deferred<unknown>();
    const dislike = deferred<unknown>();
    invokeMock.mockImplementation((command: string, args?: { action?: string }) => {
      if (command === "run_playback_action") {
        if (args?.action === "next") {
          return Promise.resolve({
            action: "next",
            current_wallpaper_path: currentPath,
            rating: null,
            paused: null,
          });
        }
        return args?.action === "like" ? like.promise : dislike.promise;
      }
      if (command === "get_wallpaper_by_path") return Promise.resolve(wallpaper(currentPath, currentRating));
      if (command === "get_image_metadata") return Promise.resolve({ width: 1920, height: 1080, file_size: 42 });
      if (command === "get_shell_metadata") return Promise.resolve({ authors: "", copyright: "", comment: "" });
      if (command === "load_preview_image") return Promise.resolve(null);
      if (command === "get_stats") return Promise.resolve(emptyStats);
      if (command === "get_yearly_stats") return Promise.resolve(emptyYearlyStats);
      return Promise.resolve(undefined);
    });
    const store = useWallpaperStore();
    await store.nextWallpaper();
    await flush();
    invokeMock.mockClear();
    const first = store.likeCurrentWallpaper();
    const second = store.dislikeCurrentWallpaper();

    await flush();
    expect(invokeMock.mock.calls.filter(([command]) => command === "run_playback_action")).toEqual([
      ["run_playback_action", { action: "like" }],
    ]);
    currentRating = 1;
    like.resolve({ action: "like", current_wallpaper_path: currentPath, rating: 1, paused: null });
    await first;
    await flush();
    currentRating = -1;
    dislike.resolve({ action: "dislike", current_wallpaper_path: currentPath, rating: -1, paused: null });
    await second;

    expect(store.currentWallpaperPath).toBe(currentPath);
    expect(store.currentWallpaper?.rating).toBe(-1);
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_wallpaper_by_path"),
    ).toHaveLength(2);
  });

  it("continues local command serialization after an invoke rejection", async () => {
    let playbackCalls = 0;
    invokeMock.mockImplementation(async (command: string, args?: { action?: string }) => {
      if (command === "run_playback_action") {
        playbackCalls += 1;
        if (playbackCalls === 1) throw new Error("backend unavailable");
        return { action: args?.action, current_wallpaper_path: null, rating: null, paused: true };
      }
      return undefined;
    });
    const store = useWallpaperStore();

    await store.togglePause();
    await store.togglePause();

    expect(playbackCalls).toBe(2);
    expect(store.isPaused).toBe(true);
  });

  it("registers caller-filterable playback completion listeners on the current WebviewWindow", async () => {
    const store = useWallpaperStore();
    await store.setupListeners();

    const target = { target: { kind: "WebviewWindow", label: "main" } };
    for (const eventName of ["pause-changed", "auto-rotated", "wallpaper-rating-changed"]) {
      expect(listenMock).toHaveBeenCalledWith(eventName, expect.any(Function), target);
    }
    expect(
      listenMock.mock.calls.filter(
        ([eventName, _handler, options]) =>
          ["pause-changed", "auto-rotated", "wallpaper-rating-changed"].includes(eventName) &&
          options === undefined,
      ),
    ).toHaveLength(0);
    expect(currentWebviewWindowMock).toHaveBeenCalledOnce();

    expect(listenMock.mock.calls.map(([eventName]) => eventName)).toContain("wallpaper-rating-changed");
    expect(listenMock.mock.calls.map(([eventName]) => eventName)).not.toContain("tray-like");
    expect(listenMock.mock.calls.map(([eventName]) => eventName)).not.toContain("tray-dislike");
  });

  it("refreshes library sources when a source sync failure event arrives", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "list_library_sources") {
        return [
          {
            path: "D:\\Walls",
            source: "mounted",
            status: "offline",
            available_count: 0,
            unavailable_count: 1,
            last_scan_at: null,
            last_error: "backend unavailable",
          },
        ];
      }
      return undefined;
    });
    const store = useWallpaperStore();
    await store.setupListeners();
    invokeMock.mockClear();

    await listenerHandlers.get("operation-failed")?.({
      payload: {
        title: "Folder update failed",
        message: "backend unavailable",
        kind: "source-sync",
        source_path: "D:\\Walls",
      },
    });
    await flush();

    expect(invokeMock).toHaveBeenCalledWith("list_library_sources");
    expect(store.librarySources[0]).toMatchObject({
      path: "D:\\Walls",
      status: "offline",
      last_error: "backend unavailable",
    });
  });

  it("keeps path-targeted gallery and inspector ratings on their legacy commands", async () => {
    const store = useWallpaperStore();
    const path = "D:/walls/selected.jpg";

    await store.like(path);
    await store.dislike(path);
    await store.resetRating(path);

    expect(invokeMock).toHaveBeenCalledWith("like_wallpaper", { path });
    expect(invokeMock).toHaveBeenCalledWith("dislike_wallpaper", { path });
    expect(invokeMock).toHaveBeenCalledWith("reset_rating", { path });
  });

  it("routes widget playback separately from hide and normalizes every error shape", async () => {
    const widgetInvoke = vi.fn<(...args: [string, Record<string, unknown>?]) => Promise<unknown>>();
    const controller = createWidgetCommandController(widgetInvoke);

    widgetInvoke.mockResolvedValueOnce(undefined);
    await controller.runPlaybackAction("like");
    expect(widgetInvoke).toHaveBeenCalledWith("run_playback_action", { action: "like" });

    widgetInvoke.mockRejectedValueOnce({ message: " structured failure " });
    await controller.runPlaybackAction("next");
    expect(controller.errorMessage).toBe("structured failure");

    widgetInvoke.mockRejectedValueOnce(" string failure ");
    await controller.runPlaybackAction("dislike");
    expect(controller.errorMessage).toBe("string failure");

    widgetInvoke.mockRejectedValueOnce(new Error("   "));
    await controller.runPlaybackAction("like");
    expect(controller.errorMessage).toBe(getWidgetCommandErrorMessage(undefined));

    widgetInvoke.mockResolvedValueOnce(undefined);
    await controller.runPlaybackAction("like");
    expect(controller.errorMessage).toBe("");

    widgetInvoke.mockResolvedValueOnce(undefined);
    await controller.hideWidget();
    expect(widgetInvoke).toHaveBeenLastCalledWith("hide_widget");
  });

  it("keeps Next current state when a delayed rating completion targets the prior wallpaper", async () => {
    const firstPath = "D:/walls/a.jpg";
    const nextPath = "D:/walls/b.jpg";
    invokeMock.mockImplementation(async (command: string, args?: { action?: string; path?: string }) => {
      if (command === "get_wallpapers_page") {
        return {
          items: [wallpaper(firstPath), wallpaper(nextPath)],
          total: 2,
          offset: 0,
          limit: 60,
          has_more: false,
        };
      }
      if (command === "run_playback_action") {
        return {
          action: args?.action,
          current_wallpaper_path: nextPath,
          rating: null,
          paused: null,
        };
      }
      if (command === "get_wallpaper_by_path") {
        const path = args?.path ?? nextPath;
        return wallpaper(path, path === firstPath ? 1 : 0);
      }
      if (command === "get_image_metadata") {
        return args?.path === nextPath
          ? { width: 3840, height: 2160, file_size: 84 }
          : { width: 1280, height: 720, file_size: 21 };
      }
      if (command === "get_shell_metadata") {
        return args?.path === nextPath
          ? { authors: "B artist", copyright: "B", comment: "B metadata" }
          : { authors: "A artist", copyright: "A", comment: "A metadata" };
      }
      if (command === "load_preview_image") return null;
      if (command === "get_stats") return emptyStats;
      if (command === "get_yearly_stats") return emptyYearlyStats;
      return undefined;
    });
    const store = useWallpaperStore();
    await store.loadWallpapers();
    await flush();
    await store.setupListeners();

    await store.nextWallpaper();
    await flush();
    expect(store.currentWallpaperPath).toBe(nextPath);
    expect(store.activeWallpaperPath).toBe(nextPath);
    expect(store.currentWallpaper?.path).toBe(nextPath);
    expect(store.activeMediaState.path).toBe(nextPath);
    expect(store.activeMetadata).toEqual({ width: 3840, height: 2160, file_size: 84 });
    expect(store.activeShellMetadata).toEqual({
      authors: "B artist",
      copyright: "B",
      comment: "B metadata",
    });

    invokeMock.mockClear();
    listenerHandlers.get("wallpaper-rating-changed")?.({
      payload: { path: firstPath, rating: 1 },
    });
    await flush();
    await flush();

    expect(store.currentWallpaperPath).toBe(nextPath);
    expect(store.activeWallpaperPath).toBe(nextPath);
    expect(store.currentWallpaper?.path).toBe(nextPath);
    expect(store.activeMediaState.path).toBe(nextPath);
    expect(store.activeMetadata).toEqual({ width: 3840, height: 2160, file_size: 84 });
    expect(store.activeShellMetadata).toEqual({
      authors: "B artist",
      copyright: "B",
      comment: "B metadata",
    });
    expect(store.wallpapers.find((entry) => entry.path === firstPath)?.rating).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith("get_stats");
    expect(invokeMock).toHaveBeenCalledWith("get_yearly_stats", { year: 2026 });
    expect(invokeMock).not.toHaveBeenCalledWith("load_preview_image", { path: firstPath });
    expect(invokeMock).not.toHaveBeenCalledWith("get_image_metadata", { path: firstPath });
    expect(invokeMock).not.toHaveBeenCalledWith("get_shell_metadata", { path: firstPath });
  });
});
