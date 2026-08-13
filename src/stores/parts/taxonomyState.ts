// Taxonomy + batch mutation helpers for the wallpaper store (src/stores/parts).
//
// Plain TS module — never a Pinia store and never importing ../wallpapers.
// The store passes a typed context object holding the refs/callbacks this
// module needs; the store keeps thin facade wrappers with identical names.

import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type {
  BatchMutationResult,
  BatchRefreshTarget,
  CollectionEntry,
  DeleteResult,
  FilterKey,
  NotifyFn,
  ReportFailureFn,
  TagEntry,
  WallpaperEntry,
  WallpaperSnapshot,
} from "./types";

const DELETE_UNDO_MS = 7000;
export const DEFAULT_TAG_COLOR = "#0a84ff";

export interface TaxonomyStateContext {
  wallpapers: Ref<WallpaperEntry[]>;
  tags: Ref<TagEntry[]>;
  collections: Ref<CollectionEntry[]>;
  currentFilter: Ref<FilterKey>;
  selectedPaths: Set<string>;
  isBatchMutating: Ref<boolean>;
  thumbnails: Map<string, string>;
  previews: Map<string, string>;
  notify: NotifyFn;
  reportFailure: ReportFailureFn;
  patchWallpaper: (path: string, patch: Partial<WallpaperEntry>) => void;
  snapshotWallpapers: (paths: string[]) => WallpaperSnapshot[];
  restoreSnapshots: (snapshots: WallpaperSnapshot[]) => void;
  removePaths: (paths: string[]) => void;
  clearSelection: () => void;
  loadWallpapers: () => Promise<void>;
  loadStats: () => Promise<void>;
  loadCollections: () => Promise<void>;
}

export async function like(path: string, ctx: TaxonomyStateContext) {
  try {
    await invoke("like_wallpaper", { path });
    ctx.patchWallpaper(path, { rating: 1 });
    await ctx.loadStats();
  } catch (e) {
    ctx.reportFailure("Failed to like wallpaper", e);
  }
}

export async function dislike(path: string, ctx: TaxonomyStateContext) {
  try {
    await invoke("dislike_wallpaper", { path });
    ctx.patchWallpaper(path, { rating: -1 });
    await ctx.loadStats();
  } catch (e) {
    ctx.reportFailure("Failed to dislike wallpaper", e);
  }
}

export async function resetRating(path: string, ctx: TaxonomyStateContext) {
  try {
    await invoke("reset_rating", { path });
    ctx.patchWallpaper(path, { rating: 0 });
    await ctx.loadStats();
  } catch (e) {
    ctx.reportFailure("Failed to reset rating", e);
  }
}

export async function saveDisplayTitle(
  path: string,
  displayTitle: string,
  ctx: TaxonomyStateContext,
) {
  try {
    const wallpaper = await invoke<WallpaperEntry>("set_wallpaper_display_title", {
      path,
      displayTitle,
    });
    ctx.patchWallpaper(path, wallpaper);
    return wallpaper;
  } catch (e) {
    ctx.reportFailure("Failed to save title", e);
    return null;
  }
}

export async function setBlacklisted(
  path: string,
  blacklisted: boolean,
  ctx: TaxonomyStateContext,
) {
  try {
    await invoke("set_blacklisted", { path, blacklisted });
    if ((blacklisted && ctx.currentFilter.value !== "blacklisted") || (!blacklisted && ctx.currentFilter.value === "blacklisted")) {
      ctx.removePaths([path]);
    } else {
      ctx.patchWallpaper(path, { blacklisted });
    }
    ctx.selectedPaths.delete(path);
    await ctx.loadStats();
    if (blacklisted) {
      ctx.notify("Wallpaper hidden", "Moved to Hidden.", "info", {
        label: "Undo",
        run: () => restoreHidden([path], ctx),
      });
    }
  } catch (e) {
    ctx.reportFailure("Failed to update hidden state", e);
  }
}

export async function deleteWallpaper(path: string, ctx: TaxonomyStateContext) {
  const snapshots = ctx.snapshotWallpapers([path]);
  if (snapshots.length === 0) return;

  ctx.removePaths([path]);
  ctx.thumbnails.delete(path);
  ctx.previews.delete(path);
  ctx.selectedPaths.delete(path);

  const timeout = window.setTimeout(() => {
    void commitPendingDelete([path], snapshots, ctx);
  }, DELETE_UNDO_MS);

  ctx.notify("Wallpaper queued for deletion", "It will move to the Recycle Bin shortly.", "info", {
    label: "Undo",
    run: () => {
      window.clearTimeout(timeout);
      ctx.restoreSnapshots(snapshots);
    },
  });
}

export async function reconcileDeleteResults(
  results: DeleteResult[],
  snapshots: WallpaperSnapshot[],
  ctx: TaxonomyStateContext,
) {
  const notRecycled = new Set(
    results
      .filter((result) => result.status === "rejected" || result.status === "not_recycled")
      .map((result) => result.path),
  );
  if (notRecycled.size > 0) {
    ctx.restoreSnapshots(snapshots.filter((snapshot) => notRecycled.has(snapshot.wallpaper.path)));
    ctx.notify("Some wallpapers were not deleted", `${notRecycled.size} items stayed in your library.`, "error");
  }

  const unknown = results.filter((result) => result.status === "recycle_outcome_unknown");
  if (unknown.length > 0) {
    ctx.notify(
      "Recycle Bin outcome needs attention",
      unknown[0].message ?? `${unknown.length} wallpaper deletion outcome(s) could not be confirmed.`,
      "error",
    );
  }

  const cleanupFailures = results.filter(
    (result) => result.status === "recycled_metadata_cleanup_failed",
  );
  if (cleanupFailures.length > 0) {
    ctx.notify(
      "Deletion metadata cleanup warning",
      cleanupFailures[0].message ?? "Some recycled wallpapers still have local metadata.",
      "error",
    );
  }

  // Reconcile every view after any structured result. Recycled items remain removed;
  // only known-not-recycled outcomes restore their optimistic snapshots above.
  await Promise.all([ctx.loadWallpapers(), ctx.loadStats(), ctx.loadCollections()]);
}

export async function commitPendingDelete(
  paths: string[],
  snapshots: WallpaperSnapshot[],
  ctx: TaxonomyStateContext,
) {
  let results: DeleteResult[];
  try {
    results = paths.length === 1
      ? [await invoke<DeleteResult>("delete_wallpaper", { path: paths[0] })]
      : await invoke<DeleteResult[]>("batch_delete_wallpapers", { paths });
  } catch (e) {
    // The backend guarantees that an invocation error before a structured result
    // means no Recycle Bin call began, so restoring all optimistic snapshots is safe.
    ctx.restoreSnapshots(snapshots);
    ctx.reportFailure("Failed to delete wallpaper", e);
    return;
  }

  try {
    await reconcileDeleteResults(results, snapshots, ctx);
  } catch (e) {
    // A structured result crosses the post-effect boundary. Reconciliation has
    // already restored only known-not-recycled items; never restore all snapshots
    // after a refresh failure because confirmed/unknown Shell effects may exist.
    ctx.reportFailure("Deletion completed, but library refresh failed", e);
  }
}

export async function createTag(
  name: string,
  color: string,
  ctx: TaxonomyStateContext,
) {
  try {
    const tag = await invoke<TagEntry>("create_tag", { name, color });
    if (!ctx.tags.value.some((t) => t.id === tag.id)) {
      ctx.tags.value = [...ctx.tags.value, tag].sort((a, b) => a.name.localeCompare(b.name));
    }
    return tag;
  } catch (e) {
    ctx.reportFailure("Failed to create tag", e);
    return null;
  }
}

export async function deleteTag(tagId: number, ctx: TaxonomyStateContext) {
  try {
    await invoke("delete_tag", { tagId });
    ctx.tags.value = ctx.tags.value.filter((tag) => tag.id !== tagId);
    if (ctx.currentFilter.value === `tag:${tagId}`) {
      ctx.currentFilter.value = "all";
    }
    await ctx.loadWallpapers();
  } catch (e) {
    ctx.reportFailure("Failed to delete tag", e);
  }
}

export async function createCollection(
  name: string,
  color: string,
  ctx: TaxonomyStateContext,
) {
  try {
    const collection = await invoke<CollectionEntry>("create_collection", { name, color });
    if (!ctx.collections.value.some((item) => item.id === collection.id)) {
      ctx.collections.value = [...ctx.collections.value, collection].sort((a, b) => a.name.localeCompare(b.name));
    }
    return collection;
  } catch (e) {
    ctx.reportFailure("Failed to create collection", e);
    return null;
  }
}

export async function assignCollection(
  path: string,
  collectionId: number,
  ctx: TaxonomyStateContext,
) {
  try {
    await invoke("assign_collection", { path, collectionId });
    await ctx.loadCollections();
  } catch (e) {
    ctx.reportFailure("Failed to add wallpaper to collection", e);
  }
}

export async function unassignCollection(
  path: string,
  collectionId: number,
  ctx: TaxonomyStateContext,
) {
  try {
    await invoke("unassign_collection", { path, collectionId });
    await ctx.loadCollections();
  } catch (e) {
    ctx.reportFailure("Failed to remove wallpaper from collection", e);
  }
}

export async function assignTag(path: string, tagId: number, ctx: TaxonomyStateContext) {
  try {
    await invoke("assign_tag", { path, tagId });
    const tag = ctx.tags.value.find((t) => t.id === tagId);
    const wp = ctx.wallpapers.value.find((w) => w.path === path);
    if (tag && wp && !wp.tags.some((t) => t.id === tagId)) {
      ctx.patchWallpaper(path, { tags: [...wp.tags, tag] });
    }
  } catch (e) {
    ctx.reportFailure("Failed to assign tag", e);
  }
}

export async function unassignTag(path: string, tagId: number, ctx: TaxonomyStateContext) {
  try {
    await invoke("unassign_tag", { path, tagId });
    const wp = ctx.wallpapers.value.find((w) => w.path === path);
    if (wp) {
      ctx.patchWallpaper(path, { tags: wp.tags.filter((tag) => tag.id !== tagId) });
    }
  } catch (e) {
    ctx.reportFailure("Failed to remove tag", e);
  }
}

async function refreshBatchTargets(
  targets: BatchRefreshTarget[],
  ctx: TaxonomyStateContext,
) {
  const refresh = new Set(targets);
  const pending: Promise<void>[] = [];
  if (refresh.has("wallpapers")) pending.push(ctx.loadWallpapers());
  if (refresh.has("stats")) pending.push(ctx.loadStats());
  if (refresh.has("collections")) pending.push(ctx.loadCollections());
  await Promise.all(pending);
}

export async function runBatchMutation(
  command: string,
  args: Record<string, unknown>,
  failureTitle: string,
  ctx: TaxonomyStateContext,
  onSuccess?: (paths: string[]) => void | Promise<void>,
): Promise<boolean> {
  const paths = Array.from(ctx.selectedPaths);
  if (paths.length === 0 || ctx.isBatchMutating.value) return false;

  ctx.isBatchMutating.value = true;
  try {
    const result = await invoke<BatchMutationResult>(command, { paths, ...args });
    await onSuccess?.(paths);
    ctx.clearSelection();
    await refreshBatchTargets(result.refresh, ctx);
    return true;
  } catch (error) {
    ctx.reportFailure(failureTitle, error);
    return false;
  } finally {
    ctx.isBatchMutating.value = false;
  }
}

export async function batchSetRating(rating: number, ctx: TaxonomyStateContext) {
  return runBatchMutation(
    "batch_set_rating",
    { rating },
    "Failed to update selected ratings",
    ctx,
    (paths) => {
      for (const path of paths) ctx.patchWallpaper(path, { rating });
    },
  );
}

export async function batchAssignTag(tagId: number, ctx: TaxonomyStateContext) {
  if (!tagId) return false;
  return runBatchMutation(
    "batch_assign_tag",
    { tagId },
    "Failed to tag selected wallpapers",
    ctx,
  );
}

export async function batchUnassignTag(tagId: number, ctx: TaxonomyStateContext) {
  if (!tagId) return false;
  return runBatchMutation(
    "batch_unassign_tag",
    { tagId },
    "Failed to remove tag from selected wallpapers",
    ctx,
  );
}

export async function batchAssignCollection(
  collectionId: number,
  ctx: TaxonomyStateContext,
) {
  if (!collectionId) return false;
  return runBatchMutation(
    "batch_assign_collection",
    { collectionId },
    "Failed to add selected wallpapers to collection",
    ctx,
  );
}

export async function batchUnassignCollection(
  collectionId: number,
  ctx: TaxonomyStateContext,
) {
  if (!collectionId) return false;
  return runBatchMutation(
    "batch_unassign_collection",
    { collectionId },
    "Failed to remove selected wallpapers from collection",
    ctx,
  );
}

export async function batchBlacklist(
  blacklisted: boolean,
  ctx: TaxonomyStateContext,
) {
  return runBatchMutation(
    "batch_blacklist",
    { blacklisted },
    "Failed to update selected hidden state",
    ctx,
    (paths) => {
      if ((blacklisted && ctx.currentFilter.value !== "blacklisted") || (!blacklisted && ctx.currentFilter.value === "blacklisted")) {
        ctx.removePaths(paths);
      } else {
        for (const path of paths) ctx.patchWallpaper(path, { blacklisted });
      }
      if (blacklisted) {
        ctx.notify("Wallpapers hidden", `${paths.length} wallpapers moved to Hidden.`, "info", {
          label: "Undo",
          run: () => restoreHidden(paths, ctx),
        });
      }
    },
  );
}

export async function batchDelete(ctx: TaxonomyStateContext) {
  const paths = Array.from(ctx.selectedPaths);
  if (paths.length === 0) return;
  const snapshots = ctx.snapshotWallpapers(paths);
  ctx.removePaths(paths);
  for (const path of paths) {
    ctx.thumbnails.delete(path);
    ctx.previews.delete(path);
  }
  ctx.clearSelection();

  const timeout = window.setTimeout(() => {
    void commitPendingDelete(paths, snapshots, ctx);
  }, DELETE_UNDO_MS);

  ctx.notify("Wallpapers queued for deletion", `${paths.length} items will move to the Recycle Bin shortly.`, "info", {
    label: "Undo",
    run: () => {
      window.clearTimeout(timeout);
      ctx.restoreSnapshots(snapshots);
    },
  });
}

export async function restoreHidden(paths: string[], ctx: TaxonomyStateContext) {
  try {
    await invoke("batch_blacklist", { paths, blacklisted: false });
    await ctx.loadWallpapers();
    await ctx.loadStats();
  } catch (e) {
    ctx.reportFailure("Failed to restore hidden wallpapers", e);
  }
}
