// Playback state helpers for the wallpaper store (src/stores/parts).
//
// Plain TS module — never a Pinia store and never importing ../wallpapers.
// The store passes a typed context object holding the refs/callbacks this
// module needs; the store keeps thin facade wrappers with identical names.

import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type {
  DisplayInfo,
  DisplayMode,
  FocusModeStatus,
  PlaybackAction,
  PlaybackActionOutcome,
  ReportFailureFn,
  WallpaperEntry,
} from "./types";

export interface PlaybackStateContext {
  currentWallpaperPath: Ref<string>;
  activeWallpaperPath: Ref<string>;
  currentWallpaper: Ref<WallpaperEntry | null>;
  isPaused: Ref<boolean>;
  displays: Ref<DisplayInfo[]>;
  displayMode: Ref<DisplayMode>;
  isChangingDisplayMode: Ref<boolean>;
  displayModeError: Ref<string>;
  focusMode: Ref<FocusModeStatus>;
  playbackCommandQueue: { value: Promise<void> };
  playbackReconciliationQueue: { value: Promise<void> };
  reportFailure: ReportFailureFn;
  loadPreview: (path: string) => Promise<void>;
  refreshWallpaper: (path: string) => Promise<WallpaperEntry | null>;
  loadMetadata: (path: string) => Promise<void>;
  loadShellMetadata: (path: string) => Promise<void>;
  loadStats: () => Promise<void>;
  loadYearlyStats: () => Promise<void>;
  patchWallpaper: (path: string, patch: Partial<WallpaperEntry>) => void;
}

export async function reconcilePlaybackCompletion(
  outcome: PlaybackActionOutcome,
  ctx: PlaybackStateContext,
) {
  const targetPath = outcome.current_wallpaper_path;
  if (outcome.action === "next" && targetPath) {
    ctx.currentWallpaperPath.value = targetPath;
    ctx.activeWallpaperPath.value = targetPath;
    if (ctx.currentWallpaper.value?.path !== targetPath) ctx.currentWallpaper.value = null;
    await ctx.loadPreview(targetPath);
    await ctx.refreshWallpaper(targetPath);
    await ctx.loadMetadata(targetPath);
    await ctx.loadShellMetadata(targetPath);
    await Promise.all([ctx.loadStats(), ctx.loadYearlyStats()]);
  }

  if (
    (outcome.action === "like" || outcome.action === "dislike") &&
    targetPath &&
    outcome.rating !== null
  ) {
    ctx.patchWallpaper(targetPath, { rating: outcome.rating });
    if (targetPath === ctx.currentWallpaperPath.value) {
      await ctx.refreshWallpaper(targetPath);
    }
    await Promise.all([ctx.loadStats(), ctx.loadYearlyStats()]);
  }

  if (outcome.action === "toggle_pause" && outcome.paused !== null) {
    ctx.isPaused.value = outcome.paused;
  }
}

export function enqueuePlaybackReconciliation(
  outcome: PlaybackActionOutcome,
  ctx: PlaybackStateContext,
) {
  const reconciliation = ctx.playbackReconciliationQueue.value.then(
    () => reconcilePlaybackCompletion(outcome, ctx),
    () => reconcilePlaybackCompletion(outcome, ctx),
  );
  ctx.playbackReconciliationQueue.value = reconciliation.catch(() => undefined);
  return reconciliation;
}

export function runPlaybackAction(
  action: PlaybackAction,
  ctx: PlaybackStateContext,
) {
  const command = ctx.playbackCommandQueue.value.then(async () => {
    try {
      const outcome = await invoke<PlaybackActionOutcome>("run_playback_action", { action });
      await enqueuePlaybackReconciliation(outcome, ctx);
    } catch (e) {
      ctx.reportFailure(`Failed to ${action.replace("_", " ")}`, e);
    }
  });
  ctx.playbackCommandQueue.value = command.catch(() => undefined);
  return command;
}

export async function loadPauseState(ctx: PlaybackStateContext) {
  try {
    ctx.isPaused.value = await invoke<boolean>("is_paused");
  } catch (e) {
    ctx.reportFailure("Failed to load pause state", e, false);
  }
}

export async function loadDisplays(ctx: PlaybackStateContext) {
  try {
    ctx.displays.value = await invoke("get_displays");
  } catch (e) {
    ctx.reportFailure("Failed to load displays", e, false);
    ctx.displays.value = [];
  }
}

export async function loadDisplayMode(ctx: PlaybackStateContext) {
  try {
    ctx.displayMode.value = await invoke<DisplayMode>("get_display_mode");
  } catch (e) {
    ctx.reportFailure("Failed to load display mode", e, false);
  }
}

export async function setDisplayMode(mode: DisplayMode, ctx: PlaybackStateContext) {
  if (mode === ctx.displayMode.value || ctx.isChangingDisplayMode.value) return;
  ctx.isChangingDisplayMode.value = true;
  ctx.displayModeError.value = "";
  try {
    ctx.displayMode.value = await invoke<DisplayMode>("set_display_mode", { mode });
  } catch (e) {
    ctx.reportFailure("Failed to set display mode", e);
    ctx.displayModeError.value = e instanceof Error ? e.message : String(e);
  } finally {
    ctx.isChangingDisplayMode.value = false;
  }
}

export async function loadFocusModeStatus(ctx: PlaybackStateContext) {
  try {
    ctx.focusMode.value = await invoke("get_focus_mode_status");
  } catch (e) {
    ctx.reportFailure("Failed to load focus mode", e, false);
  }
}

export async function setFocusModeEnabled(enabled: boolean, ctx: PlaybackStateContext) {
  try {
    ctx.focusMode.value = await invoke("set_focus_mode_enabled", { enabled });
    ctx.isPaused.value = ctx.focusMode.value.auto_paused ? true : ctx.isPaused.value;
  } catch (e) {
    ctx.reportFailure("Failed to set focus mode", e);
  }
}
