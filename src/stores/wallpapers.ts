import { computed, reactive, ref, watch } from "vue";
import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useNotifications } from "../composables/useNotifications";
import {
  chooseActiveWallpaperPath,
  createActiveMediaState,
  type ActiveMediaState,
} from "./activeMedia";
import * as mediaState from "./parts/mediaState";
import type { MediaStateContext } from "./parts/mediaState";
import * as playbackState from "./parts/playbackState";
import type { PlaybackStateContext } from "./parts/playbackState";
import * as sourcesState from "./parts/sourcesState";
import type { SourcesStateContext } from "./parts/sourcesState";
import * as taxonomyState from "./parts/taxonomyState";
import { DEFAULT_TAG_COLOR } from "./parts/taxonomyState";
import type { TaxonomyStateContext } from "./parts/taxonomyState";
import type {
  ActiveWallpaperBootstrap,
  BackupClientSettings,
  BackupExportResult,
  BackupImportPreview,
  BackupImportResult,
  BackupMergeResult,
  BatchMutationResult,
  BatchRefreshTarget,
  CollectionEntry,
  DeleteResult,
  DisplayInfo,
  DisplayMode,
  FilterKey,
  FocusModeStatus,
  ImageMetadata,
  ImportResult,
  LibrarySource,
  LibrarySourceMutation,
  LibrarySourceOperation,
  LibrarySourceStatus,
  MonthlyPlayStats,
  OperationFailedPayload,
  PlaybackAction,
  PlaybackActionOutcome,
  RatingChangedPayload,
  RemoveLibrarySourceImpact,
  RemoveLibrarySourceMode,
  ShellMetadata,
  SortMode,
  Stats,
  TagEntry,
  ThumbnailGeneratedPayload,
  ThumbnailGenerationFailedPayload,
  TopWallpaperStats,
  WallpaperEntry,
  WallpaperPage,
  WallpaperSnapshot,
  WallpaperViewMode,
  WorkspaceSection,
  YearlyStats,
} from "./parts/types";

// Public type facade — components and models keep importing these names from
// this module (frozen surface). The definitions live in ./parts/types.
export type {
  BackupClientSettings,
  BackupExportResult,
  BackupImportPreview,
  BackupImportResult,
  BackupMergeResult,
  BatchMutationResult,
  BatchRefreshTarget,
  CollectionEntry,
  DeleteResult,
  DisplayInfo,
  DisplayMode,
  FilterKey,
  FocusModeStatus,
  ImageMetadata,
  ImportResult,
  LibrarySource,
  LibrarySourceMutation,
  LibrarySourceOperation,
  LibrarySourceStatus,
  MonthlyPlayStats,
  PlaybackAction,
  PlaybackActionOutcome,
  RemoveLibrarySourceImpact,
  RemoveLibrarySourceMode,
  ShellMetadata,
  SortMode,
  Stats,
  TagEntry,
  TopWallpaperStats,
  WallpaperEntry,
  WallpaperPage,
  WallpaperViewMode,
  WorkspaceSection,
  YearlyStats,
};

const WALLPAPER_PAGE_SIZE = 96;
const SEARCH_RELOAD_DEBOUNCE_MS = 220;

export const useWallpaperStore = defineStore("wallpapers", () => {
  const { notify, notifyError } = useNotifications();
  let listenersReady = false;
  let listenersSetupPromise: Promise<void> | null = null;
  const listenerUnsubscribers: UnlistenFn[] = [];
  const wallpapers = ref<WallpaperEntry[]>([]);
  const wallpaperIndexByPath = new Map<string, number>();
  const tags = ref<TagEntry[]>([]);
  const collections = ref<CollectionEntry[]>([]);
  const librarySources = ref<LibrarySource[]>([]);
  const librarySourceBusy = reactive<Map<string, LibrarySourceOperation>>(new Map());
  const librarySourceErrors = reactive<Map<string, string>>(new Map());
  const thumbnails = reactive<Map<string, string>>(new Map());
  const previews = reactive<Map<string, string>>(new Map());
  const pendingThumbnailLoads = new Set<string>();
  const thumbnailErrors = reactive<Map<string, string>>(new Map());
  const thumbnailRetryAttempts = new Map<string, number>();
  const thumbnailRetryTimers = new Map<string, number>();
  const previewLoadPromises = new Map<string, Promise<void>>();
  const imageMetadata = reactive<Map<string, ImageMetadata>>(new Map());
  const shellMetadata = reactive<Map<string, ShellMetadata>>(new Map());
  const selectedPaths = reactive<Set<string>>(new Set());
  const currentFilter = ref<FilterKey>("all");
  const sortMode = ref<SortMode>("created");
  const wallpaperViewMode = ref<WallpaperViewMode>("grid");
  const workspaceSection = ref<WorkspaceSection>("library");
  const searchQuery = ref("");
  const wallpaperTotal = ref(0);
  const nextWallpaperOffset = ref(0);
  const hasMoreWallpapers = ref(false);
  const isLoadingMoreWallpapers = ref(false);
  const selectionMode = ref(false);
  const isBatchMutating = ref(false);
  const isLoading = ref(true);
  const isImporting = ref(false);
  const isBackupBusy = ref(false);
  const backupError = ref("");
  const isLoadingCliLog = ref(false);
  const isPaused = ref(false);
  const cliLog = ref("");
  const activeWallpaperPath = ref("");
  const displayMode = ref<DisplayMode>("all");
  const isChangingDisplayMode = ref(false);
  const displayModeError = ref("");
  const displays = ref<DisplayInfo[]>([]);
  const focusMode = ref<FocusModeStatus>({
    enabled: false,
    fullscreen_detected: false,
    auto_paused: false,
  });
  const yearlyStats = ref<YearlyStats>({
    year: new Date().getFullYear(),
    total_plays: 0,
    unique_wallpapers: 0,
    liked_plays: 0,
    monthly: Array.from({ length: 12 }, (_, index) => ({ month: index + 1, plays: 0 })),
    top_wallpapers: [],
  });
  const stats = ref<Stats>({
    total: 0,
    liked: 0,
    disliked: 0,
    blacklisted: 0,
    total_plays: 0,
  });
  const currentWallpaperPath = ref("");
  const currentWallpaper = ref<WallpaperEntry | null>(null);
  const bootstrappedWallpaper = ref<WallpaperEntry | null>(null);
  const activeMediaState = ref<ActiveMediaState>(createActiveMediaState());
  const playbackCommandQueue = { value: Promise.resolve() as Promise<void> };
  const playbackReconciliationQueue = { value: Promise.resolve() as Promise<void> };
  const activePreviewLoadKey = { value: "" };
  const speculativeWarmGeneration = { value: 0 };
  let wallpaperPageRequestId = 0;
  let searchReloadTimer: number | undefined;
  const selectedCount = computed(() => selectedPaths.size);
  const visibleWallpapers = computed(() => wallpapers.value);
  const activeMediaPath = computed(
    () => activeWallpaperPath.value || currentWallpaperPath.value,
  );
  const activeWallpaper = computed(() => {
    const preferredPath = activeMediaPath.value;
    if (!preferredPath) return null;
    return (
      wallpapers.value.find((w) => w.path === preferredPath) ||
      (currentWallpaper.value?.path === preferredPath
        ? currentWallpaper.value
        : null) ||
      (bootstrappedWallpaper.value?.path === preferredPath
        ? bootstrappedWallpaper.value
        : null)
    );
  });
  const activeThumbnailUrl = computed(() =>
    activeMediaPath.value
      ? thumbnails.get(activeMediaPath.value) || ""
      : "",
  );
  const activePreviewCandidateUrl = computed(() =>
    activeMediaPath.value ? previews.get(activeMediaPath.value) || "" : "",
  );
  const activeDisplayUrl = computed(() => activeMediaState.value.displayUrl);
  // Kept as a compatibility alias while all preview surfaces migrate to activeDisplayUrl.
  const activePreviewUrl = activeDisplayUrl;
  const activeMetadata = computed(() => {
    const active = activeWallpaper.value;
    if (!active) return null;
    return (
      imageMetadata.get(active.path) ||
      (active.width > 0 || active.height > 0 || active.file_size > 0
        ? { width: active.width, height: active.height, file_size: active.file_size }
        : null)
    );
  });
  const activeShellMetadata = computed(() => {
    const active = activeWallpaper.value;
    return active ? shellMetadata.get(active.path) || null : null;
  });

  function reportFailure(title: string, error: unknown, visible = true) {
    console.error(`${title}:`, error);
    if (visible) {
      notifyError(title, error);
    }
  }

  // Typed internal API handed to the extracted parts modules. The parts never
  // import this store; they receive the refs/callbacks they need here.
  const mediaCtx: MediaStateContext = {
    wallpapers,
    wallpaperIndexByPath,
    bootstrappedWallpaper,
    thumbnails,
    previews,
    pendingThumbnailLoads,
    thumbnailErrors,
    thumbnailRetryAttempts,
    thumbnailRetryTimers,
    previewLoadPromises,
    imageMetadata,
    shellMetadata,
    activeMediaState,
    activeWallpaperPath,
    currentWallpaperPath,
    activeMediaPath,
    activeThumbnailUrl,
    activePreviewCandidateUrl,
    activeWallpaper,
    activePreviewLoadKey,
    speculativeWarmGeneration,
    reportFailure,
    patchWallpaper,
  };

  const playbackCtx: PlaybackStateContext = {
    currentWallpaperPath,
    activeWallpaperPath,
    currentWallpaper,
    isPaused,
    displays,
    displayMode,
    isChangingDisplayMode,
    displayModeError,
    focusMode,
    playbackCommandQueue,
    playbackReconciliationQueue,
    reportFailure,
    loadPreview,
    refreshWallpaper,
    loadMetadata,
    loadShellMetadata,
    loadStats,
    loadYearlyStats,
    patchWallpaper,
  };

  const taxonomyCtx: TaxonomyStateContext = {
    wallpapers,
    tags,
    collections,
    currentFilter,
    selectedPaths,
    isBatchMutating,
    thumbnails,
    previews,
    notify,
    reportFailure,
    patchWallpaper,
    snapshotWallpapers,
    restoreSnapshots,
    removePaths,
    clearSelection,
    loadWallpapers,
    loadStats,
    loadCollections,
  };

  const sourcesCtx: SourcesStateContext = {
    librarySourceBusy,
    librarySourceErrors,
    isBackupBusy,
    backupError,
    reportFailure,
    notify,
    loadLibrarySources,
    loadWallpapers,
    loadStats,
    loadTags,
    loadCollections,
    loadDisplayMode,
    loadFocusModeStatus,
    loadPauseState,
  };

  function rebuildWallpaperIndex() {
    wallpaperIndexByPath.clear();
    wallpapers.value.forEach((wallpaper, index) => {
      wallpaperIndexByPath.set(wallpaper.path, index);
    });
  }

  function setWallpapers(nextWallpapers: WallpaperEntry[]) {
    wallpapers.value = nextWallpapers;
    rebuildWallpaperIndex();
  }

  function loadedPageSearch() {
    return searchQuery.value.trim();
  }

  function applyWallpaperPage(page: WallpaperPage, append: boolean) {
    wallpaperTotal.value = page.total;
    nextWallpaperOffset.value = Math.max(0, page.offset + page.limit);
    hasMoreWallpapers.value = page.has_more;

    if (!append) {
      setWallpapers(page.items);
      mediaState.scheduleSpeculativePreviewWarmup(mediaCtx);
      return;
    }

    const knownPaths = new Set(wallpapers.value.map((wallpaper) => wallpaper.path));
    const nextWallpapers = [...wallpapers.value];
    for (const item of page.items) {
      if (!knownPaths.has(item.path)) {
        knownPaths.add(item.path);
        nextWallpapers.push(item);
      }
    }
    setWallpapers(nextWallpapers);
    mediaState.scheduleSpeculativePreviewWarmup(mediaCtx);
  }

  async function bootstrapActiveWallpaper() {
    try {
      const bootstrap = await invoke<ActiveWallpaperBootstrap>(
        "bootstrap_active_wallpaper",
      );
      if (!bootstrap.path || !bootstrap.wallpaper) return;

      currentWallpaperPath.value = bootstrap.path;
      activeWallpaperPath.value = bootstrap.path;
      bootstrappedWallpaper.value = bootstrap.wallpaper;
      currentWallpaper.value = bootstrap.wallpaper;

      if (bootstrap.thumbnail_path) {
        mediaState.setMediaCacheEntry(
          thumbnails,
          bootstrap.path,
          mediaState.toAssetUrl(bootstrap.thumbnail_path),
          mediaState.MAX_THUMBNAIL_CACHE_ENTRIES,
          mediaCtx,
        );
      }
      if (bootstrap.preview_path) {
        mediaState.setMediaCacheEntry(
          previews,
          bootstrap.path,
          mediaState.toAssetUrl(bootstrap.preview_path),
          mediaState.MAX_PREVIEW_CACHE_ENTRIES,
          mediaCtx,
        );
      }
    } catch (e) {
      reportFailure("Failed to restore current wallpaper", e, false);
    }
  }

  async function loadWallpapers() {
    const requestId = ++wallpaperPageRequestId;
    isLoading.value = true;
    isLoadingMoreWallpapers.value = false;
    try {
      const page = await invoke<WallpaperPage>("get_wallpapers_page", {
        filter: currentFilter.value,
        sort: sortMode.value,
        search: loadedPageSearch(),
        offset: 0,
        limit: WALLPAPER_PAGE_SIZE,
      });
      if (requestId !== wallpaperPageRequestId) return;

      applyWallpaperPage(page, false);
      pruneSelection();
      ensureActiveWallpaper();
      mediaState.scheduleActivePreviewLoad(mediaCtx);
      void loadActiveMetadata();
      void loadActiveShellMetadata();
    } catch (e) {
      if (requestId === wallpaperPageRequestId) {
        reportFailure("Failed to load wallpapers", e);
      }
    } finally {
      if (requestId === wallpaperPageRequestId) {
        isLoading.value = false;
      }
    }
  }

  async function loadMoreWallpapers() {
    if (isLoading.value || isLoadingMoreWallpapers.value || !hasMoreWallpapers.value) return;

    const requestId = wallpaperPageRequestId;
    isLoadingMoreWallpapers.value = true;
    try {
      const page = await invoke<WallpaperPage>("get_wallpapers_page", {
        filter: currentFilter.value,
        sort: sortMode.value,
        search: loadedPageSearch(),
        offset: nextWallpaperOffset.value,
        limit: WALLPAPER_PAGE_SIZE,
      });
      if (requestId !== wallpaperPageRequestId) return;
      applyWallpaperPage(page, true);
    } catch (e) {
      if (requestId === wallpaperPageRequestId) {
        reportFailure("Failed to load more wallpapers", e, false);
      }
    } finally {
      if (requestId === wallpaperPageRequestId) {
        isLoadingMoreWallpapers.value = false;
      }
    }
  }

  async function loadAllThumbnails() {
    await mediaState.loadAllThumbnails(mediaCtx);
  }

  async function loadThumbnailsForPaths(paths: string[]) {
    await mediaState.loadThumbnailsForPaths(paths, mediaCtx);
  }

  function handleThumbnailLoadError(path: string, failedUrl: string) {
    mediaState.handleThumbnailLoadError(path, failedUrl, mediaCtx);
  }

  function retryThumbnail(path: string) {
    mediaState.retryThumbnail(path, mediaCtx);
  }

  async function loadTags() {
    try {
      tags.value = await invoke("get_tags");
    } catch (e) {
      reportFailure("Failed to load tags", e);
    }
  }

  async function loadCollections() {
    try {
      collections.value = await invoke("get_collections");
    } catch (e) {
      reportFailure("Failed to load collections", e);
    }
  }

  async function setFolder(path: string) {
    isLoading.value = true;
    try {
      await invoke<ImportResult>("set_wallpaper_folder", { path });
      thumbnails.clear();
      previews.clear();
      shellMetadata.clear();
      clearSelection();
      await loadWallpapers();
      await loadStats();
    } catch (e) {
      reportFailure("Failed to set folder", e);
    } finally {
      isLoading.value = false;
    }
  }

  async function importFolder(path: string) {
    isImporting.value = true;
    try {
      // The command returns immediately now — scan + import runs on a
      // background thread. The `import-complete` event listener reloads
      // wallpapers/stats and clears isImporting when finished.
      await invoke("import_wallpaper_folder", { path });
    } catch (e) {
      reportFailure("Failed to import folder", e);
      isImporting.value = false;
      throw e;
    }
  }

  async function loadLibrarySources() {
    try {
      librarySources.value =
        await invoke<LibrarySource[]>("list_library_sources");
    } catch (error) {
      reportFailure("Failed to load library sources", error, false);
    }
  }

  async function rescanLibrarySource(path: string) {
    return sourcesState.rescanLibrarySource(path, sourcesCtx);
  }

  async function retryLibrarySource(path: string) {
    return sourcesState.retryLibrarySource(path, sourcesCtx);
  }

  async function previewRemoveLibrarySource(path: string) {
    return sourcesState.previewRemoveLibrarySource(path);
  }

  async function removeLibrarySource(
    path: string,
    mode: RemoveLibrarySourceMode,
  ) {
    return sourcesState.removeLibrarySource(path, mode, sourcesCtx);
  }

  async function relocateLibrarySource(path: string, newPath: string) {
    return sourcesState.relocateLibrarySource(path, newPath, sourcesCtx);
  }

  async function exportLibraryBackup(
    destination: string,
    clientSettings: BackupClientSettings,
  ): Promise<BackupExportResult> {
    return sourcesState.exportLibraryBackup(destination, clientSettings, sourcesCtx);
  }

  async function previewBackupImport(
    path: string,
  ): Promise<BackupImportPreview> {
    return sourcesState.previewBackupImport(path, sourcesCtx);
  }

  async function importLibraryBackup(
    path: string,
    expectedDigest: string,
  ): Promise<BackupImportResult> {
    return sourcesState.importLibraryBackup(path, expectedDigest, sourcesCtx);
  }

  async function importFiles(paths: string[]) {
    if (paths.length === 0) return { scanned: 0, imported: 0 };
    isImporting.value = true;
    try {
      const result = await invoke<ImportResult>("import_wallpaper_files", { paths });
      await loadWallpapers();
      await loadStats();
      return result;
    } catch (e) {
      reportFailure("Failed to import files", e);
      throw e;
    } finally {
      isImporting.value = false;
    }
  }

  async function importDroppedPaths(paths: string[]) {
    if (paths.length === 0) return { scanned: 0, imported: 0 };
    isImporting.value = true;
    try {
      const result = await invoke<ImportResult>("import_dropped_paths", { paths });
      await loadWallpapers();
      await loadStats();
      notify("Drop import complete", `Imported ${result.imported} wallpapers.`, "info");
      return result;
    } catch (e) {
      reportFailure("Failed to import dropped wallpapers", e);
      throw e;
    } finally {
      isImporting.value = false;
    }
  }

  function runPlaybackAction(action: PlaybackAction) {
    return playbackState.runPlaybackAction(action, playbackCtx);
  }

  async function nextWallpaper() {
    await runPlaybackAction("next");
  }

  async function likeCurrentWallpaper() {
    await runPlaybackAction("like");
  }

  async function dislikeCurrentWallpaper() {
    await runPlaybackAction("dislike");
  }

  async function setAsWallpaper(path: string) {
    try {
      await invoke("set_current_wallpaper", { path });
      currentWallpaperPath.value = path;
      currentWallpaper.value = null;
      activeWallpaperPath.value = path;
      void loadPreview(path);
      void loadMetadata(path);
      void loadShellMetadata(path);
      await refreshWallpaper(path);
      await loadStats();
      await loadYearlyStats();
    } catch (e) {
      reportFailure("Failed to set wallpaper", e);
    }
  }

  async function refreshWallpaper(path: string) {
    try {
      const wallpaper = await invoke<WallpaperEntry>("get_wallpaper_by_path", { path });
      patchWallpaper(path, wallpaper);
      if (path === currentWallpaperPath.value) currentWallpaper.value = wallpaper;
      return wallpaper;
    } catch (e) {
      reportFailure("Failed to refresh wallpaper", e, false);
      return null;
    }
  }

  function patchWallpaper(path: string, patch: Partial<WallpaperEntry>) {
    let idx = wallpaperIndexByPath.get(path);
    if (idx === undefined || wallpapers.value[idx]?.path !== path) {
      idx = wallpapers.value.findIndex((w) => w.path === path);
      if (idx !== -1) {
        wallpaperIndexByPath.set(path, idx);
      }
    }

    if (idx !== undefined && idx !== -1) {
      wallpapers.value[idx] = { ...wallpapers.value[idx], ...patch };
    }
    if (currentWallpaper.value?.path === path) {
      currentWallpaper.value = { ...currentWallpaper.value, ...patch };
    }
    if (bootstrappedWallpaper.value?.path === path) {
      bootstrappedWallpaper.value = { ...bootstrappedWallpaper.value, ...patch };
    }
  }

  function snapshotWallpapers(paths: string[]) {
    const pathSet = new Set(paths);
    return wallpapers.value
      .map((wallpaper, index) => ({ wallpaper, index }))
      .filter((snapshot) => pathSet.has(snapshot.wallpaper.path));
  }

  function restoreSnapshots(snapshots: WallpaperSnapshot[]) {
    if (snapshots.length === 0) return;

    const existing = new Set(wallpapers.value.map((wallpaper) => wallpaper.path));
    const next = [...wallpapers.value];
    for (const snapshot of [...snapshots].sort((a, b) => a.index - b.index)) {
      if (existing.has(snapshot.wallpaper.path)) continue;
      next.splice(Math.min(snapshot.index, next.length), 0, snapshot.wallpaper);
      existing.add(snapshot.wallpaper.path);
    }
    setWallpapers(next);
    ensureActiveWallpaper();
  }

  function setActiveWallpaper(path: string) {
    activeWallpaperPath.value = path;
    workspaceSection.value = "library";
    void loadPreview(path);
    void loadMetadata(path);
    void loadShellMetadata(path);
  }

  async function loadPreview(path: string) {
    await mediaState.loadPreview(path, mediaCtx);
  }

  async function loadActivePreview() {
    await mediaState.loadActivePreview(mediaCtx);
  }

  async function loadMetadata(path: string) {
    await mediaState.loadMetadata(path, mediaCtx);
  }

  async function loadActiveMetadata() {
    await mediaState.loadActiveMetadata(mediaCtx);
  }

  async function loadShellMetadata(path: string) {
    await mediaState.loadShellMetadata(path, mediaCtx);
  }

  async function loadActiveShellMetadata() {
    await mediaState.loadActiveShellMetadata(mediaCtx);
  }

  async function like(path: string) {
    return taxonomyState.like(path, taxonomyCtx);
  }

  async function dislike(path: string) {
    return taxonomyState.dislike(path, taxonomyCtx);
  }

  async function resetRating(path: string) {
    return taxonomyState.resetRating(path, taxonomyCtx);
  }

  async function saveDisplayTitle(path: string, displayTitle: string) {
    return taxonomyState.saveDisplayTitle(path, displayTitle, taxonomyCtx);
  }

  async function setBlacklisted(path: string, blacklisted: boolean) {
    return taxonomyState.setBlacklisted(path, blacklisted, taxonomyCtx);
  }

  async function deleteWallpaper(path: string) {
    return taxonomyState.deleteWallpaper(path, taxonomyCtx);
  }

  async function createTag(name: string, color = DEFAULT_TAG_COLOR) {
    return taxonomyState.createTag(name, color, taxonomyCtx);
  }

  async function deleteTag(tagId: number) {
    return taxonomyState.deleteTag(tagId, taxonomyCtx);
  }

  async function createCollection(name: string, color = DEFAULT_TAG_COLOR) {
    return taxonomyState.createCollection(name, color, taxonomyCtx);
  }

  async function assignCollection(path: string, collectionId: number) {
    return taxonomyState.assignCollection(path, collectionId, taxonomyCtx);
  }

  async function unassignCollection(path: string, collectionId: number) {
    return taxonomyState.unassignCollection(path, collectionId, taxonomyCtx);
  }

  async function assignTag(path: string, tagId: number) {
    return taxonomyState.assignTag(path, tagId, taxonomyCtx);
  }

  async function unassignTag(path: string, tagId: number) {
    return taxonomyState.unassignTag(path, tagId, taxonomyCtx);
  }

  async function batchSetRating(rating: number) {
    return taxonomyState.batchSetRating(rating, taxonomyCtx);
  }

  async function batchAssignTag(tagId: number) {
    return taxonomyState.batchAssignTag(tagId, taxonomyCtx);
  }

  async function batchUnassignTag(tagId: number) {
    return taxonomyState.batchUnassignTag(tagId, taxonomyCtx);
  }

  async function batchAssignCollection(collectionId: number) {
    return taxonomyState.batchAssignCollection(collectionId, taxonomyCtx);
  }

  async function batchUnassignCollection(collectionId: number) {
    return taxonomyState.batchUnassignCollection(collectionId, taxonomyCtx);
  }

  async function batchBlacklist(blacklisted: boolean) {
    return taxonomyState.batchBlacklist(blacklisted, taxonomyCtx);
  }

  async function batchDelete() {
    return taxonomyState.batchDelete(taxonomyCtx);
  }

  async function togglePause() {
    await runPlaybackAction("toggle_pause");
  }

  async function loadPauseState() {
    await playbackState.loadPauseState(playbackCtx);
  }

  async function loadStats() {
    try {
      stats.value = await invoke("get_stats");
    } catch (e) {
      reportFailure("Failed to load stats", e, false);
    }
  }

  async function loadDisplays() {
    await playbackState.loadDisplays(playbackCtx);
  }

  async function loadDisplayMode() {
    await playbackState.loadDisplayMode(playbackCtx);
  }

  async function setDisplayMode(mode: DisplayMode) {
    await playbackState.setDisplayMode(mode, playbackCtx);
  }

  async function loadFocusModeStatus() {
    await playbackState.loadFocusModeStatus(playbackCtx);
  }

  async function setFocusModeEnabled(enabled: boolean) {
    await playbackState.setFocusModeEnabled(enabled, playbackCtx);
  }

  async function loadYearlyStats(year = yearlyStats.value.year) {
    try {
      yearlyStats.value = await invoke("get_yearly_stats", { year });
    } catch (e) {
      reportFailure("Failed to load yearly stats", e, false);
    }
  }

  async function loadPhaseFour() {
    await Promise.all([
      loadDisplays(),
      loadDisplayMode(),
      loadPauseState(),
      loadFocusModeStatus(),
      loadYearlyStats(),
    ]);
  }

  async function loadCliLog() {
    isLoadingCliLog.value = true;
    try {
      cliLog.value = await invoke<string>("read_cli_log");
    } catch (e) {
      reportFailure("Failed to load CLI diagnostics", e);
      cliLog.value = "";
    } finally {
      isLoadingCliLog.value = false;
    }
  }

  async function setFilter(filter: FilterKey) {
    workspaceSection.value = "library";
    currentFilter.value = filter;
    clearSelection();
    await loadWallpapers();
  }

  async function setSort(sort: SortMode) {
    sortMode.value = sort;
    await loadWallpapers();
  }

  function toggleSelection(path: string) {
    if (selectedPaths.has(path)) {
      selectedPaths.delete(path);
    } else {
      selectedPaths.add(path);
    }
  }

  function setWallpaperViewMode(mode: WallpaperViewMode) {
    wallpaperViewMode.value = mode;
  }

  function setWorkspaceSection(section: WorkspaceSection) {
    workspaceSection.value = section;
  }

  function clearSelection() {
    selectedPaths.clear();
    selectionMode.value = false;
  }

  function removePaths(paths: string[]) {
    const remove = new Set(paths);
    const beforeCount = wallpapers.value.length;
    setWallpapers(wallpapers.value.filter((w) => !remove.has(w.path)));
    const removedCount = beforeCount - wallpapers.value.length;
    wallpaperTotal.value = Math.max(0, wallpaperTotal.value - removedCount);
    nextWallpaperOffset.value = Math.max(0, nextWallpaperOffset.value - removedCount);
    if (activeWallpaperPath.value && remove.has(activeWallpaperPath.value)) {
      activeWallpaperPath.value = wallpapers.value[0]?.path || "";
    }
    if (currentWallpaperPath.value && remove.has(currentWallpaperPath.value)) {
      currentWallpaperPath.value = "";
      currentWallpaper.value = null;
    } else if (currentWallpaper.value && remove.has(currentWallpaper.value.path)) {
      currentWallpaper.value = null;
    }
  }

  function pruneSelection() {
    const visible = new Set(wallpapers.value.map((w) => w.path));
    for (const path of selectedPaths) {
      if (!visible.has(path)) selectedPaths.delete(path);
    }
  }

  function ensureActiveWallpaper() {
    activeWallpaperPath.value = chooseActiveWallpaperPath({
      activePath: activeWallpaperPath.value,
      currentPath: currentWallpaperPath.value,
      loadedPaths: wallpapers.value.map((wallpaper) => wallpaper.path),
    });
  }

  async function setupListeners() {
    if (listenersReady) return;
    if (listenersSetupPromise) {
      await listenersSetupPromise;
      return;
    }

    listenersSetupPromise = (async () => {
      const playbackCompletionTarget = {
        target: { kind: "WebviewWindow" as const, label: getCurrentWebviewWindow().label },
      };

      const unsubscribers = await Promise.all([
        listen("folder-changed", async () => {
          await Promise.all([
            loadWallpapers(),
            loadStats(),
            loadLibrarySources(),
          ]);
        }),
        listen<boolean>("pause-changed", (event) => {
          void playbackState.enqueuePlaybackReconciliation(
            { action: "toggle_pause", current_wallpaper_path: null, rating: null, paused: event.payload },
            playbackCtx,
          );
        }, playbackCompletionTarget),
        listen<string>("auto-rotated", (event) => {
          void playbackState.enqueuePlaybackReconciliation(
            { action: "next", current_wallpaper_path: event.payload, rating: null, paused: null },
            playbackCtx,
          );
        }, playbackCompletionTarget),
        listen<RatingChangedPayload>("wallpaper-rating-changed", (event) => {
          const action = event.payload.rating === 1 ? "like" : "dislike";
          void playbackState.enqueuePlaybackReconciliation(
            { action, current_wallpaper_path: event.payload.path, rating: event.payload.rating, paused: null },
            playbackCtx,
          );
        }, playbackCompletionTarget),
        listen<FocusModeStatus>("focus-mode-changed", (event) => {
          focusMode.value = event.payload;
        }),
        listen<ThumbnailGeneratedPayload>("thumbnail-generated", (event) => {
          mediaState.cacheThumbnailPath(event.payload.path, event.payload.cache_path, mediaCtx);
        }),
        listen<ThumbnailGenerationFailedPayload>(
          "thumbnail-generation-failed",
          (event) => {
            mediaState.handleThumbnailGenerationFailure(
              event.payload.path,
              event.payload.message,
              mediaCtx,
            );
          },
        ),
        listen<ThumbnailGeneratedPayload>("preview-generated", (event) => {
          const { path, cache_path } = event.payload;
          const url = mediaState.toAssetUrl(cache_path);
          console.debug("[PureWall] preview-generated event:", path, "→", url);
          mediaState.setMediaCacheEntry(previews, path, url, mediaState.MAX_PREVIEW_CACHE_ENTRIES, mediaCtx);
          previewLoadPromises.delete(path);
        }),
        listen<ImportResult>("import-complete", async (event) => {
          isImporting.value = false;
          await loadLibrarySources();
          if (event.payload.imported > 0) {
            await Promise.all([
              loadWallpapers(),
              loadStats(),
            ]);
            notify(
              "Import complete",
              `Imported ${event.payload.imported} wallpapers.`,
              "info",
            );
          }
        }),
        listen<OperationFailedPayload>("operation-failed", (event) => {
          notify(event.payload.title, event.payload.message, "error");
        }),
      ]);

      listenerUnsubscribers.push(...unsubscribers);
      listenersReady = true;
    })();

    try {
      await listenersSetupPromise;
    } finally {
      listenersSetupPromise = null;
    }
  }

  function teardownListeners() {
    window.clearTimeout(searchReloadTimer);
    for (const timer of thumbnailRetryTimers.values()) {
      window.clearTimeout(timer);
    }
    thumbnailRetryTimers.clear();
    for (const unlisten of listenerUnsubscribers.splice(0)) {
      unlisten();
    }
    listenersReady = false;
    listenersSetupPromise = null;
  }

  watch(searchQuery, () => {
    window.clearTimeout(searchReloadTimer);
    searchReloadTimer = window.setTimeout(() => {
      void loadWallpapers();
    }, SEARCH_RELOAD_DEBOUNCE_MS);
  });

  watch(
    [activeMediaPath, activeThumbnailUrl, activePreviewCandidateUrl],
    () => mediaState.syncActiveMediaPresentation(mediaCtx),
    { immediate: true, flush: "sync" },
  );

  return {
    wallpapers,
    tags,
    collections,
    librarySources,
    librarySourceBusy,
    librarySourceErrors,
    thumbnails,
    thumbnailErrors,
    previews,
    imageMetadata,
    shellMetadata,
    selectedPaths,
    selectedCount,
    visibleWallpapers,
    wallpaperTotal,
    hasMoreWallpapers,
    isLoadingMoreWallpapers,
    currentFilter,
    sortMode,
    wallpaperViewMode,
    workspaceSection,
    searchQuery,
    selectionMode,
    isBatchMutating,
    isLoading,
    isImporting,
    isBackupBusy,
    backupError,
    isLoadingCliLog,
    isPaused,
    cliLog,
    activeWallpaperPath,
    displayMode,
    isChangingDisplayMode,
    displayModeError,
    displays,
    focusMode,
    yearlyStats,
    stats,
    currentWallpaperPath,
    currentWallpaper,
    activeWallpaper,
    activeMediaState,
    activeThumbnailUrl,
    activeDisplayUrl,
    activePreviewUrl,
    activeMetadata,
    activeShellMetadata,
    bootstrapActiveWallpaper,
    loadWallpapers,
    loadMoreWallpapers,
    loadAllThumbnails,
    loadThumbnailsForPaths,
    handleThumbnailLoadError,
    retryThumbnail,
    loadTags,
    loadCollections,
    setFolder,
    importFolder,
    loadLibrarySources,
    rescanLibrarySource,
    retryLibrarySource,
    previewRemoveLibrarySource,
    removeLibrarySource,
    relocateLibrarySource,
    exportLibraryBackup,
    previewBackupImport,
    importLibraryBackup,
    importFiles,
    importDroppedPaths,
    runPlaybackAction,
    nextWallpaper,
    setAsWallpaper,
    like,
    dislike,
    resetRating,
    likeCurrentWallpaper,
    dislikeCurrentWallpaper,
    setActiveWallpaper,
    loadPreview,
    loadActivePreview,
    loadMetadata,
    loadActiveMetadata,
    loadShellMetadata,
    loadActiveShellMetadata,
    saveDisplayTitle,
    setBlacklisted,
    deleteWallpaper,
    createTag,
    deleteTag,
    createCollection,
    assignCollection,
    unassignCollection,
    assignTag,
    unassignTag,
    batchSetRating,
    batchAssignTag,
    batchUnassignTag,
    batchAssignCollection,
    batchUnassignCollection,
    batchBlacklist,
    batchDelete,
    togglePause,
    loadPauseState,
    loadStats,
    loadDisplays,
    loadDisplayMode,
    setDisplayMode,
    loadFocusModeStatus,
    setFocusModeEnabled,
    loadYearlyStats,
    loadPhaseFour,
    loadCliLog,
    setFilter,
    setSort,
    setWallpaperViewMode,
    setWorkspaceSection,
    toggleSelection,
    clearSelection,
    setupListeners,
    teardownListeners,
  };
});
