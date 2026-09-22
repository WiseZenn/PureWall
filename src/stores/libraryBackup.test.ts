import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const {
  currentWebviewWindowMock,
  invokeMock,
  listenMock,
  notifyErrorMock,
  notifyMock,
} = vi.hoisted(() => ({
  currentWebviewWindowMock: vi.fn(),
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  notifyErrorMock: vi.fn(),
  notifyMock: vi.fn(),
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

vi.mock("../composables/useNotifications", () => ({
  useNotifications: () => ({
    notify: notifyMock,
    notifyError: notifyErrorMock,
  }),
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

const preview = {
  contentDigest: "sha256:previewed-backup",
  sourceCount: 2,
  wallpaperCount: 7,
  tagCount: 3,
  collectionCount: 1,
  settingCount: 4,
  newWallpapers: 5,
  overwrittenWallpapers: 2,
  missingPaths: 1,
  warnings: ["1 wallpaper path is offline."],
};

const importResult = {
  committed: true,
  merge: {
    addedWallpapers: 5,
    updatedWallpapers: 2,
    addedSources: 1,
    createdTags: 1,
    createdCollections: 0,
    updatedSettings: 3,
    clientSettings: {
      theme: "dark",
      workspaceMode: "quiet",
    },
  },
  warnings: ["D:\\Offline could not start a watcher."],
};

describe("library backup store routing", () => {
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
    notifyMock.mockReset();
    notifyErrorMock.mockReset();
  });

  it("routes export with exact client settings and reports success", async () => {
    invokeMock.mockResolvedValueOnce({
      path: "D:\\Backups\\purewall.json",
      bytes: 2048,
    });
    const store = useWallpaperStore();

    await expect(
      store.exportLibraryBackup("D:\\Backups\\purewall.json", {
        theme: "dark",
        workspaceMode: "workbench",
      }),
    ).resolves.toEqual({
      path: "D:\\Backups\\purewall.json",
      bytes: 2048,
    });

    expect(invokeMock).toHaveBeenCalledWith("export_library_backup", {
      destination: "D:\\Backups\\purewall.json",
      clientSettings: {
        theme: "dark",
        workspaceMode: "workbench",
      },
    });
    expect(notifyMock).toHaveBeenCalledWith(
      "Backup exported",
      expect.stringContaining("D:\\Backups\\purewall.json"),
      "success",
    );
    expect(store.isBackupBusy).toBe(false);
    expect(store.backupError).toBe("");
  });

  it("routes preview exactly and leaves gallery selection unchanged", async () => {
    invokeMock.mockResolvedValueOnce(preview);
    const store = useWallpaperStore();
    store.selectionMode = true;
    store.selectedPaths.add("D:\\Walls\\selected.jpg");

    await expect(
      store.previewBackupImport("D:\\Backups\\purewall.json"),
    ).resolves.toEqual(preview);

    expect(invokeMock).toHaveBeenCalledWith("preview_backup_import", {
      path: "D:\\Backups\\purewall.json",
    });
    expect(store.selectedPaths.has("D:\\Walls\\selected.jpg")).toBe(true);
    expect(store.selectionMode).toBe(true);
  });

  it("binds import confirmation to the exact previewed content digest", async () => {
    invokeMock.mockResolvedValueOnce(importResult);
    const store = useWallpaperStore();

    await store.importLibraryBackup(
      "D:\\Backups\\purewall.json",
      preview.contentDigest,
    );

    expect(invokeMock).toHaveBeenCalledWith("import_library_backup", {
      path: "D:\\Backups\\purewall.json",
      expectedDigest: preview.contentDigest,
    });
  });

  it("rejects overlapping backup operations with one shared busy state", async () => {
    let resolveExport: ((value: unknown) => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      if (command === "export_library_backup") {
        return new Promise((resolve) => {
          resolveExport = resolve;
        });
      }
      return Promise.resolve(preview);
    });
    const store = useWallpaperStore();

    const first = store.exportLibraryBackup("D:\\Backups\\purewall.json", {
      theme: "system",
      workspaceMode: "workbench",
    });
    expect(store.isBackupBusy).toBe(true);
    await expect(
      store.previewBackupImport("D:\\Backups\\purewall.json"),
    ).rejects.toThrow("already in progress");
    expect(
      invokeMock.mock.calls.filter(
        ([command]) => command === "preview_backup_import",
      ),
    ).toHaveLength(0);

    resolveExport?.({
      path: "D:\\Backups\\purewall.json",
      bytes: 1024,
    });
    await first;
    expect(store.isBackupBusy).toBe(false);
  });

  it("retains actionable errors, releases busy state, and preserves selection", async () => {
    const store = useWallpaperStore();
    store.selectionMode = true;
    store.selectedPaths.add("D:\\Walls\\selected.jpg");

    invokeMock.mockRejectedValueOnce({
      code: "BACKUP_INVALID_DATA",
      message: "The backup contains an undefined tag.",
    });
    await expect(
      store.previewBackupImport("D:\\Backups\\broken.json"),
    ).rejects.toMatchObject({ code: "BACKUP_INVALID_DATA" });
    expect(store.backupError).toBe(
      "The backup contains an undefined tag.",
    );
    expect(store.isBackupBusy).toBe(false);
    expect(store.selectedPaths.has("D:\\Walls\\selected.jpg")).toBe(true);

    invokeMock.mockRejectedValueOnce({
      code: "BACKUP_WRITE_FAILED",
      message: "The backup file disappeared.",
    });
    await expect(
      store.importLibraryBackup(
        "D:\\Backups\\missing.json",
        preview.contentDigest,
      ),
    ).rejects.toMatchObject({ code: "BACKUP_WRITE_FAILED" });
    expect(store.backupError).toBe("The backup file disappeared.");
    expect(store.isBackupBusy).toBe(false);
    expect(store.selectedPaths.has("D:\\Walls\\selected.jpg")).toBe(true);
    expect(store.selectionMode).toBe(true);
  });

  it("refreshes every owner after commit and reports success with warnings", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "import_library_backup") return importResult;
      if (command === "list_library_sources") return [];
      if (command === "get_wallpapers_page") return emptyPage;
      if (command === "get_tags") return [];
      if (command === "get_collections") return [];
      if (command === "get_stats") return emptyStats;
      if (command === "get_display_mode") return "span";
      if (command === "get_focus_mode_status") {
        return {
          enabled: true,
          fullscreen_detected: false,
          auto_paused: false,
        };
      }
      if (command === "is_paused") return false;
      return undefined;
    });
    const store = useWallpaperStore();

    await expect(
      store.importLibraryBackup(
        "D:\\Backups\\purewall.json",
        preview.contentDigest,
      ),
    ).resolves.toEqual(importResult);

    expect(invokeMock).toHaveBeenCalledWith("import_library_backup", {
      path: "D:\\Backups\\purewall.json",
      expectedDigest: preview.contentDigest,
    });
    const commands = invokeMock.mock.calls.map(([command]) => command);
    expect(commands).toEqual(
      expect.arrayContaining([
        "list_library_sources",
        "get_wallpapers_page",
        "get_tags",
        "get_collections",
        "get_stats",
        "get_display_mode",
        "get_focus_mode_status",
        "is_paused",
      ]),
    );
    expect(notifyMock).toHaveBeenCalledWith(
      "Backup imported",
      expect.stringContaining("committed"),
      "success",
    );
    expect(notifyMock).toHaveBeenCalledWith(
      "Backup imported with warnings",
      "D:\\Offline could not start a watcher.",
      "info",
    );
    expect(store.displayMode).toBe("span");
    expect(store.focusMode.enabled).toBe(true);
    expect(store.isPaused).toBe(false);
    expect(store.isBackupBusy).toBe(false);
  });
});
