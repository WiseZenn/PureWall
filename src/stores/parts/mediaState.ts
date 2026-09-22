// Media state helpers for the wallpaper store (src/stores/parts).
//
// Plain TS module — never a Pinia store and never importing ../wallpapers.
// The store passes a typed context object holding the refs/callbacks this
// module needs; all public store properties/actions keep their names there.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import { nextThumbnailRetry } from "../../utils/thumbnailRetryPolicy";
import {
  beginActiveMedia,
  chooseSpeculativePreviewPaths,
  previewAvailable,
  previewFailed,
  previewLoaded,
  thumbnailAvailable,
  type ActiveMediaState,
} from "../activeMedia";
import type {
  ImageMetadata,
  ReportFailureFn,
  ShellMetadata,
  WallpaperEntry,
} from "./types";

export const MAX_THUMBNAIL_CACHE_ENTRIES = 600;
export const MAX_PREVIEW_CACHE_ENTRIES = 24;
const THUMBNAIL_BATCH_SIZE = 12;
const THUMBNAIL_CACHE_WRITE_CHUNK_SIZE = 6;

export interface MediaStateContext {
  wallpapers: Ref<WallpaperEntry[]>;
  wallpaperIndexByPath: Map<string, number>;
  bootstrappedWallpaper: Ref<WallpaperEntry | null>;
  thumbnails: Map<string, string>;
  previews: Map<string, string>;
  pendingThumbnailLoads: Set<string>;
  thumbnailErrors: Map<string, string>;
  thumbnailRetryAttempts: Map<string, number>;
  thumbnailRetryTimers: Map<string, number>;
  previewLoadPromises: Map<string, Promise<void>>;
  imageMetadata: Map<string, ImageMetadata>;
  shellMetadata: Map<string, ShellMetadata>;
  activeMediaState: Ref<ActiveMediaState>;
  activeWallpaperPath: Ref<string>;
  currentWallpaperPath: Ref<string>;
  activeMediaPath: Ref<string>;
  activeThumbnailUrl: Ref<string>;
  activePreviewCandidateUrl: Ref<string>;
  activeWallpaper: Ref<WallpaperEntry | null>;
  activePreviewLoadKey: { value: string };
  speculativeWarmGeneration: { value: number };
  reportFailure: ReportFailureFn;
  patchWallpaper: (path: string, patch: Partial<WallpaperEntry>) => void;
}

export function toAssetUrl(filePath: string) {
  // Normalize backslashes to forward slashes for consistent asset protocol URLs
  const normalized = filePath.replace(/\\/g, "/");
  const url = convertFileSrc(normalized);
  return url;
}

export function touchMediaCacheEntry(cache: Map<string, string>, path: string) {
  const cached = cache.get(path);
  if (cached === undefined) return false;

  cache.delete(path);
  cache.set(path, cached);
  return true;
}

function protectedMediaPaths(ctx: MediaStateContext) {
  return [ctx.activeWallpaperPath.value, ctx.currentWallpaperPath.value].filter(Boolean);
}

export function setMediaCacheEntry(
  cache: Map<string, string>,
  path: string,
  dataUrl: string,
  maxEntries: number,
  ctx: MediaStateContext,
) {
  if (cache.has(path)) {
    cache.delete(path);
  }
  cache.set(path, dataUrl);
  pruneMediaCache(cache, maxEntries, ctx);
}

export function pruneMediaCache(
  cache: Map<string, string>,
  maxEntries: number,
  ctx: MediaStateContext,
) {
  const protectedPaths = new Set(protectedMediaPaths(ctx));

  for (const path of protectedPaths) {
    touchMediaCacheEntry(cache, path);
  }

  for (const path of cache.keys()) {
    if (cache.size <= maxEntries) break;
    if (!protectedPaths.has(path)) {
      cache.delete(path);
    }
  }
}

function waitForNextFrame() {
  return new Promise<void>((resolve) => {
    if (typeof window.requestAnimationFrame === "function") {
      window.requestAnimationFrame(() => resolve());
    } else {
      window.setTimeout(resolve, 0);
    }
  });
}

function wallpaperIsInLibrary(path: string, ctx: MediaStateContext) {
  if (ctx.wallpaperIndexByPath.has(path)) return true;
  if (ctx.bootstrappedWallpaper.value?.path === path) {
    return true;
  }
  // Fallback: search the array directly (index may be stale after rapid filter changes)
  return ctx.wallpapers.value.some((wallpaper) => wallpaper.path === path);
}

function clearThumbnailRetryTimer(path: string, ctx: MediaStateContext) {
  const timer = ctx.thumbnailRetryTimers.get(path);
  if (timer === undefined) return;
  window.clearTimeout(timer);
  ctx.thumbnailRetryTimers.delete(path);
}

function resetThumbnailFailure(path: string, ctx: MediaStateContext) {
  clearThumbnailRetryTimer(path, ctx);
  ctx.thumbnailRetryAttempts.delete(path);
  ctx.thumbnailErrors.delete(path);
}

export function handleThumbnailGenerationFailure(
  path: string,
  message: string,
  ctx: MediaStateContext,
) {
  ctx.pendingThumbnailLoads.delete(path);
  if (!path || !wallpaperIsInLibrary(path, ctx) || ctx.thumbnails.has(path)) {
    resetThumbnailFailure(path, ctx);
    return;
  }

  clearThumbnailRetryTimer(path, ctx);
  const retry = nextThumbnailRetry(ctx.thumbnailRetryAttempts.get(path) ?? 0);
  if (!retry) {
    ctx.thumbnailErrors.set(path, message || "Thumbnail could not be generated");
    return;
  }

  ctx.thumbnailRetryAttempts.set(path, retry.attempt);
  ctx.thumbnailErrors.delete(path);
  const timer = window.setTimeout(() => {
    ctx.thumbnailRetryTimers.delete(path);
    if (!wallpaperIsInLibrary(path, ctx) || ctx.thumbnails.has(path)) {
      resetThumbnailFailure(path, ctx);
      return;
    }
    void loadThumbnailsForPaths([path], ctx);
  }, retry.delayMs);
  ctx.thumbnailRetryTimers.set(path, timer);
}

export function handleThumbnailLoadError(
  path: string,
  failedUrl: string,
  ctx: MediaStateContext,
) {
  if (!failedUrl || ctx.thumbnails.get(path) !== failedUrl) return;
  ctx.thumbnails.delete(path);
  handleThumbnailGenerationFailure(path, "Cached thumbnail could not be displayed", ctx);
}

export function retryThumbnail(path: string, ctx: MediaStateContext) {
  ctx.pendingThumbnailLoads.delete(path);
  resetThumbnailFailure(path, ctx);
  void loadThumbnailsForPaths([path], ctx);
}

export function cacheThumbnailPath(
  path: string,
  cachePath: string,
  ctx: MediaStateContext,
) {
  if (!path || !cachePath) return;
  // Verify the wallpaper is still in the current library before caching.
  // Use a soft check — the index may be briefly out of sync during filter/sort changes.
  if (!wallpaperIsInLibrary(path, ctx)) {
    console.debug(`[PureWall] Skipping thumbnail cache for removed/unknown wallpaper: ${path}`);
    ctx.pendingThumbnailLoads.delete(path);
    resetThumbnailFailure(path, ctx);
    return;
  }
  const assetUrl = toAssetUrl(cachePath);
  console.debug(`[PureWall] Caching thumbnail: ${path} → ${assetUrl}`);
  setMediaCacheEntry(ctx.thumbnails, path, assetUrl, MAX_THUMBNAIL_CACHE_ENTRIES, ctx);
  ctx.pendingThumbnailLoads.delete(path);
  resetThumbnailFailure(path, ctx);
}

export async function commitThumbnailResult(
  result: Record<string, string>,
  ctx: MediaStateContext,
) {
  const entries = Object.entries(result);
  for (let index = 0; index < entries.length; index += THUMBNAIL_CACHE_WRITE_CHUNK_SIZE) {
    for (const [path, cachePath] of entries.slice(index, index + THUMBNAIL_CACHE_WRITE_CHUNK_SIZE)) {
      cacheThumbnailPath(path, cachePath, ctx);
    }
    if (index + THUMBNAIL_CACHE_WRITE_CHUNK_SIZE < entries.length) {
      await waitForNextFrame();
    }
  }
}

export function scheduleSpeculativePreviewWarmup(ctx: MediaStateContext) {
  const active = ctx.activeMediaState.value;
  if (
    active.phase !== "preview" ||
    !active.path ||
    active.generation === ctx.speculativeWarmGeneration.value
  ) {
    return;
  }

  const paths = chooseSpeculativePreviewPaths(
    active.path,
    ctx.wallpapers.value.map((wallpaper) => wallpaper.path),
  );
  if (paths.length === 0) return;

  const generation = active.generation;
  ctx.speculativeWarmGeneration.value = generation;
  void invoke<number>("prewarm_preview_images", { paths }).catch((error) => {
    if (ctx.speculativeWarmGeneration.value === generation) {
      ctx.speculativeWarmGeneration.value = 0;
    }
    console.debug("[PureWall] Speculative preview warmup skipped:", error);
  });
}

function preloadActivePreviewCandidate(ctx: MediaStateContext) {
  const snapshot = ctx.activeMediaState.value;
  const previewUrl = snapshot.pendingPreviewUrl;
  if (!previewUrl) return;

  const generation = snapshot.generation;
  const loadKey = `${generation}:${previewUrl}`;
  if (ctx.activePreviewLoadKey.value === loadKey) return;
  ctx.activePreviewLoadKey.value = loadKey;

  const image = new Image();
  image.decoding = "async";
  image.onload = () => {
    if (ctx.activePreviewLoadKey.value === loadKey) {
      ctx.activePreviewLoadKey.value = "";
    }
    ctx.activeMediaState.value = previewLoaded(
      ctx.activeMediaState.value,
      generation,
      previewUrl,
    );
    scheduleSpeculativePreviewWarmup(ctx);
  };
  image.onerror = () => {
    if (ctx.activePreviewLoadKey.value === loadKey) {
      ctx.activePreviewLoadKey.value = "";
    }
    if (ctx.activeMediaState.value.pendingPreviewUrl !== previewUrl) return;
    ctx.activeMediaState.value = previewFailed(
      ctx.activeMediaState.value,
      generation,
      `Failed to decode preview: ${previewUrl}`,
    );
    console.warn("[PureWall] Active preview decode failed:", snapshot.path, previewUrl);
  };
  image.src = previewUrl;
}

export function syncActiveMediaPresentation(ctx: MediaStateContext) {
  const path = ctx.activeMediaPath.value;
  const thumbnailUrl = ctx.activeThumbnailUrl.value;
  const previewUrl = ctx.activePreviewCandidateUrl.value;
  let next = ctx.activeMediaState.value;

  if (next.path !== path) {
    next = beginActiveMedia(next, { path, thumbnailUrl, previewUrl });
  } else {
    next = thumbnailAvailable(next, path, thumbnailUrl);
    next = previewAvailable(next, path, previewUrl);
  }

  ctx.activeMediaState.value = next;
  preloadActivePreviewCandidate(ctx);
}

export function scheduleActivePreviewLoad(ctx: MediaStateContext) {
  const path = ctx.activeWallpaper.value?.path;
  if (!path) return;
  void loadPreview(path, ctx);
}

export async function loadThumbnailsForPaths(
  paths: string[],
  ctx: MediaStateContext,
) {
  const pendingPaths = Array.from(new Set(paths)).filter(
    (path) =>
      path &&
      !ctx.thumbnails.has(path) &&
      !ctx.pendingThumbnailLoads.has(path) &&
      !ctx.thumbnailErrors.has(path),
  );

  if (pendingPaths.length === 0) return;

  for (const path of pendingPaths) {
    ctx.pendingThumbnailLoads.add(path);
  }

  try {
    await waitForNextFrame();
    for (let index = 0; index < pendingPaths.length; index += THUMBNAIL_BATCH_SIZE) {
      const batch = pendingPaths
        .slice(index, index + THUMBNAIL_BATCH_SIZE)
        .filter((path) => !ctx.thumbnails.has(path));
      if (batch.length === 0) continue;

      const result = await invoke<Record<string, string>>("load_thumbnails_batch", { paths: batch });
      await commitThumbnailResult(result, ctx);
      if (index + THUMBNAIL_BATCH_SIZE < pendingPaths.length) {
        await waitForNextFrame();
      }
    }
  } catch (e) {
    ctx.reportFailure("Failed to load thumbnails", e, false);
    const message = e instanceof Error ? e.message : String(e);
    for (const path of pendingPaths) {
      handleThumbnailGenerationFailure(path, message, ctx);
    }
  } finally {
    for (const path of pendingPaths) {
      ctx.pendingThumbnailLoads.delete(path);
    }
  }
}

export async function loadAllThumbnails(ctx: MediaStateContext) {
  await loadThumbnailsForPaths(ctx.wallpapers.value.map((w) => w.path), ctx);
}

export async function loadPreview(path: string, ctx: MediaStateContext) {
  if (!path) {
    console.warn("[PureWall] loadPreview skipped — empty path");
    return;
  }
  if (touchMediaCacheEntry(ctx.previews, path)) {
    console.debug("[PureWall] loadPreview cache hit in memory:", path);
    return;
  }

  const existingLoad = ctx.previewLoadPromises.get(path);
  if (existingLoad) {
    console.debug("[PureWall] loadPreview already pending:", path);
    await existingLoad;
    return;
  }

  const loadPromise = (async () => {
    try {
      const cachePath = await invoke<string | null>("load_preview_image", { path });
      if (!cachePath) {
        console.debug("[PureWall] loadPreview queued:", path);
        return;
      }

      const url = toAssetUrl(cachePath);
      console.debug("[PureWall] loadPreview:", path, "→", url);
      setMediaCacheEntry(ctx.previews, path, url, MAX_PREVIEW_CACHE_ENTRIES, ctx);
    } catch (e) {
      console.error("[PureWall] loadPreview FAILED:", path, e);
      ctx.reportFailure("Failed to load preview", e);
    } finally {
      ctx.previewLoadPromises.delete(path);
    }
  })();

  ctx.previewLoadPromises.set(path, loadPromise);
  await loadPromise;
}

export async function loadActivePreview(ctx: MediaStateContext) {
  if (ctx.activeWallpaper.value) {
    await loadPreview(ctx.activeWallpaper.value.path, ctx);
  }
}

export async function loadMetadata(path: string, ctx: MediaStateContext) {
  if (!path || ctx.imageMetadata.has(path)) return;
  try {
    const metadata = await invoke<ImageMetadata>("get_image_metadata", { path });
    ctx.imageMetadata.set(path, metadata);
    ctx.patchWallpaper(path, metadata);
  } catch (e) {
    ctx.reportFailure("Failed to load image metadata", e, false);
  }
}

export async function loadActiveMetadata(ctx: MediaStateContext) {
  if (ctx.activeWallpaper.value) {
    await loadMetadata(ctx.activeWallpaper.value.path, ctx);
  }
}

export async function loadShellMetadata(path: string, ctx: MediaStateContext) {
  if (!path || ctx.shellMetadata.has(path)) return;
  try {
    const metadata = await invoke<ShellMetadata>("get_shell_metadata", { path });
    ctx.shellMetadata.set(path, metadata);
  } catch (e) {
    ctx.reportFailure("Failed to load Windows shell metadata", e, false);
    ctx.shellMetadata.set(path, {
      authors: "",
      copyright: "",
      comment: "",
    });
  }
}

export async function loadActiveShellMetadata(ctx: MediaStateContext) {
  if (ctx.activeWallpaper.value) {
    await loadShellMetadata(ctx.activeWallpaper.value.path, ctx);
  }
}
