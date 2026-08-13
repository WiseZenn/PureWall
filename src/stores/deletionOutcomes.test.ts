import { describe, expect, it, vi } from "vitest";
import { ref } from "vue";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { commitPendingDelete, reconcileDeleteResults } from "./parts/taxonomyState";
import type { DeleteResult, WallpaperSnapshot } from "./parts/types";

function snapshot(path: string): WallpaperSnapshot {
  return { wallpaper: { path } as WallpaperSnapshot["wallpaper"], index: 0 };
}

function context() {
  return {
    wallpapers: ref([]),
    tags: ref([]),
    collections: ref([]),
    currentFilter: ref("all"),
    selectedPaths: new Set<string>(),
    isBatchMutating: ref(false),
    thumbnails: new Map<string, string>(),
    previews: new Map<string, string>(),
    notify: vi.fn(),
    reportFailure: vi.fn(),
    patchWallpaper: vi.fn(),
    snapshotWallpapers: vi.fn(),
    restoreSnapshots: vi.fn(),
    removePaths: vi.fn(),
    clearSelection: vi.fn(),
    loadWallpapers: vi.fn().mockResolvedValue(undefined),
    loadStats: vi.fn().mockResolvedValue(undefined),
    loadCollections: vi.fn().mockResolvedValue(undefined),
  } as any;
}

describe("structured wallpaper deletion outcomes", () => {
  it("restores only rejected and not-recycled snapshots and refreshes all views", async () => {
    const ctx = context();
    const snapshots = [snapshot("rejected.jpg"), snapshot("failed.jpg"), snapshot("moved.jpg")];
    const results: DeleteResult[] = [
      { path: "rejected.jpg", status: "rejected", code: "reparse_path", message: "reparse" },
      { path: "failed.jpg", status: "not_recycled", code: "recycle_failed", message: "denied" },
      { path: "moved.jpg", status: "recycled", code: null, message: null },
    ];

    await reconcileDeleteResults(results, snapshots, ctx);

    expect(ctx.restoreSnapshots).toHaveBeenCalledWith([snapshots[0], snapshots[1]]);
    expect(ctx.loadWallpapers).toHaveBeenCalledOnce();
    expect(ctx.loadStats).toHaveBeenCalledOnce();
    expect(ctx.loadCollections).toHaveBeenCalledOnce();
    expect(ctx.notify).toHaveBeenCalledWith(
      "Some wallpapers were not deleted",
      "2 items stayed in your library.",
      "error",
    );
  });

  it("does not restore unknown or metadata-cleanup failures", async () => {
    const ctx = context();
    const snapshots = [snapshot("unknown.jpg"), snapshot("metadata.jpg")];
    const results: DeleteResult[] = [
      {
        path: "unknown.jpg",
        status: "recycle_outcome_unknown",
        code: "recycle_outcome_unknown",
        message: "outcome unknown",
      },
      {
        path: "metadata.jpg",
        status: "recycled_metadata_cleanup_failed",
        code: "metadata_cleanup_failed",
        message: "metadata warning",
      },
    ];

    await reconcileDeleteResults(results, snapshots, ctx);

    expect(ctx.restoreSnapshots).not.toHaveBeenCalled();
    expect(ctx.notify).toHaveBeenCalledTimes(2);
    expect(ctx.loadWallpapers).toHaveBeenCalledOnce();
    expect(ctx.loadStats).toHaveBeenCalledOnce();
    expect(ctx.loadCollections).toHaveBeenCalledOnce();
  });

  it("uses the structured single-delete result and restores only a known rejection", async () => {
    const ctx = context();
    const snapshots = [snapshot("single.jpg")];
    invokeMock.mockResolvedValueOnce({
      path: "single.jpg",
      status: "rejected",
      code: "reparse_path",
      message: "reparse",
    });

    await commitPendingDelete(["single.jpg"], snapshots, ctx);

    expect(invokeMock).toHaveBeenCalledWith("delete_wallpaper", { path: "single.jpg" });
    expect(ctx.restoreSnapshots).toHaveBeenCalledWith(snapshots);
  });

  it("uses batch results without restoring confirmed or unknown effects", async () => {
    const ctx = context();
    const snapshots = [snapshot("a.jpg"), snapshot("b.jpg"), snapshot("c.jpg")];
    invokeMock.mockResolvedValueOnce([
      { path: "a.jpg", status: "recycled", code: null, message: null },
      { path: "b.jpg", status: "not_recycled", code: "recycle_failed", message: "denied" },
      { path: "c.jpg", status: "recycle_outcome_unknown", code: "unknown", message: "unknown" },
    ]);

    await commitPendingDelete(["a.jpg", "b.jpg", "c.jpg"], snapshots, ctx);

    expect(invokeMock).toHaveBeenCalledWith("batch_delete_wallpapers", {
      paths: ["a.jpg", "b.jpg", "c.jpg"],
    });
    expect(ctx.restoreSnapshots).toHaveBeenCalledWith([snapshots[1]]);
  });

  it("restores all snapshots only when single invoke fails before a structured result", async () => {
    const ctx = context();
    const snapshots = [snapshot("before-error.jpg")];
    invokeMock.mockRejectedValueOnce(new Error("pre-effect failure"));

    await commitPendingDelete(["before-error.jpg"], snapshots, ctx);

    expect(ctx.restoreSnapshots).toHaveBeenCalledWith(snapshots);
    expect(ctx.reportFailure).toHaveBeenCalledWith("Failed to delete wallpaper", expect.any(Error));
  });

  it("restores all snapshots when batch invoke fails before a structured result", async () => {
    const ctx = context();
    const snapshots = [snapshot("batch-a.jpg"), snapshot("batch-b.jpg")];
    invokeMock.mockRejectedValueOnce(new Error("batch pre-effect failure"));

    await commitPendingDelete(["batch-a.jpg", "batch-b.jpg"], snapshots, ctx);

    expect(ctx.restoreSnapshots).toHaveBeenCalledWith(snapshots);
    expect(ctx.reportFailure).toHaveBeenCalledWith("Failed to delete wallpaper", expect.any(Error));
  });

  it("does not restore a recycled snapshot when post-result refresh fails", async () => {
    const ctx = context();
    const snapshots = [snapshot("refreshed.jpg")];
    ctx.loadWallpapers.mockRejectedValueOnce(new Error("refresh failed"));
    invokeMock.mockResolvedValueOnce({
      path: "refreshed.jpg",
      status: "recycled",
      code: null,
      message: null,
    });

    await commitPendingDelete(["refreshed.jpg"], snapshots, ctx);

    expect(ctx.restoreSnapshots).not.toHaveBeenCalled();
    expect(ctx.reportFailure).toHaveBeenCalledWith(
      "Deletion completed, but library refresh failed",
      expect.any(Error),
    );
  });

  it("does not restore unknown batch outcomes when post-result refresh fails", async () => {
    const ctx = context();
    const snapshots = [snapshot("unknown-a.jpg"), snapshot("unknown-b.jpg")];
    ctx.loadStats.mockRejectedValueOnce(new Error("stats refresh failed"));
    invokeMock.mockResolvedValueOnce([
      { path: "unknown-a.jpg", status: "recycle_outcome_unknown", code: "unknown", message: "unknown" },
      { path: "unknown-b.jpg", status: "recycled", code: null, message: null },
    ]);

    await commitPendingDelete(["unknown-a.jpg", "unknown-b.jpg"], snapshots, ctx);

    expect(ctx.restoreSnapshots).not.toHaveBeenCalled();
    expect(ctx.reportFailure).toHaveBeenCalledWith(
      "Deletion completed, but library refresh failed",
      expect.any(Error),
    );
  });
});
