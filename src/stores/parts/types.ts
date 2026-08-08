// Shared type definitions for the wallpaper store and its extracted parts modules.
//
// Public type names stay reachable through `../wallpapers` (the store re-exports
// them) so the frozen store facade is unchanged. Parts modules import from here
// instead of from the store, which keeps the extraction free of circular imports.

import type { NotificationTone } from "../../composables/useNotifications";

export interface TagEntry {
  id: number;
  name: string;
  color: string;
}

export interface CollectionEntry {
  id: number;
  name: string;
  color: string;
  wallpaper_count: number;
}

export interface WallpaperEntry {
  id: number;
  path: string;
  hash: string;
  source: string;
  display_title: string;
  rating: number;
  play_count: number;
  last_played: string | null;
  created_at: string;
  blacklisted: boolean;
  width: number;
  height: number;
  file_size: number;
  tags: TagEntry[];
}

export interface WallpaperPage {
  items: WallpaperEntry[];
  total: number;
  offset: number;
  limit: number;
  has_more: boolean;
}

export interface Stats {
  total: number;
  liked: number;
  disliked: number;
  blacklisted: number;
  total_plays: number;
}

export interface DisplayInfo {
  index: number;
  id: string;
  left: number;
  top: number;
  width: number;
  height: number;
  current_wallpaper: string;
}

export interface FocusModeStatus {
  enabled: boolean;
  fullscreen_detected: boolean;
  auto_paused: boolean;
}

export interface MonthlyPlayStats {
  month: number;
  plays: number;
}

export interface TopWallpaperStats {
  path: string;
  plays: number;
  rating: number;
}

export interface YearlyStats {
  year: number;
  total_plays: number;
  unique_wallpapers: number;
  liked_plays: number;
  monthly: MonthlyPlayStats[];
  top_wallpapers: TopWallpaperStats[];
}

export interface ImportResult {
  scanned: number;
  imported: number;
}

export interface BackupClientSettings {
  theme: "system" | "light" | "dark";
  workspaceMode: "workbench" | "quiet";
}

export interface BackupExportResult {
  path: string;
  bytes: number;
}

export interface BackupImportPreview {
  contentDigest: string;
  sourceCount: number;
  wallpaperCount: number;
  tagCount: number;
  collectionCount: number;
  settingCount: number;
  newWallpapers: number;
  overwrittenWallpapers: number;
  missingPaths: number;
  warnings: string[];
}

export interface BackupMergeResult {
  addedWallpapers: number;
  updatedWallpapers: number;
  addedSources: number;
  createdTags: number;
  createdCollections: number;
  updatedSettings: number;
  clientSettings: {
    theme: BackupClientSettings["theme"] | null;
    workspaceMode: BackupClientSettings["workspaceMode"] | null;
  };
}

export interface BackupImportResult {
  committed: boolean;
  merge: BackupMergeResult;
  warnings: string[];
}

export type LibrarySourceStatus =
  | "online"
  | "offline"
  | "scanning"
  | "error";
export type RemoveLibrarySourceMode =
  | "keep_metadata"
  | "clear_metadata";
export type LibrarySourceOperation =
  | "rescan"
  | "retry"
  | "relocate"
  | "remove";

export interface LibrarySource {
  path: string;
  source: "mounted" | "imported-folder";
  status: LibrarySourceStatus;
  available_count: number;
  unavailable_count: number;
  last_scan_at: string | null;
  last_error: string | null;
}

export interface RemoveLibrarySourceImpact {
  affected_wallpapers: number;
}

export interface LibrarySourceMutation {
  affected_wallpapers: number;
  matched_wallpapers: number;
  imported_wallpapers: number;
  unavailable_wallpapers: number;
  watcher_warning: string | null;
}

export interface DeleteResult {
  path: string;
  deleted: boolean;
  message: string | null;
}

export type BatchRefreshTarget = "wallpapers" | "stats" | "collections";

export interface BatchMutationResult {
  affected: number;
  refresh: BatchRefreshTarget[];
}

export interface ImageMetadata {
  width: number;
  height: number;
  file_size: number;
}

export interface ShellMetadata {
  authors: string;
  copyright: string;
  comment: string;
}

export type FilterKey = "all" | "liked" | "disliked" | "blacklisted" | `tag:${number}` | `collection:${number}`;
export type SortMode = "created" | "liked" | "plays" | "recent";
export type DisplayMode = "all" | "span" | "independent";
export type WallpaperViewMode = "grid" | "list" | "compact";
export type WorkspaceSection = "library" | "displays" | "settings" | "shortcuts" | "advanced" | "insights";

export type PlaybackAction = "next" | "like" | "dislike" | "toggle_pause";

export interface PlaybackActionOutcome {
  action: PlaybackAction;
  current_wallpaper_path: string | null;
  rating: number | null;
  paused: boolean | null;
}

// Internal event/state payloads (not part of the public store facade).
export interface RatingChangedPayload {
  path: string;
  rating: number;
}

export interface OperationFailedPayload {
  title: string;
  message: string;
}

export interface ThumbnailGeneratedPayload {
  path: string;
  cache_path: string;
}

export interface ThumbnailGenerationFailedPayload {
  path: string;
  message: string;
}

export interface ActiveWallpaperBootstrap {
  path: string;
  wallpaper: WallpaperEntry | null;
  thumbnail_path: string | null;
  preview_path: string | null;
  preview_state: "ready" | "queued" | "unavailable";
}

export interface WallpaperSnapshot {
  wallpaper: WallpaperEntry;
  index: number;
}

export type NotifyFn = (
  title: string,
  message: string,
  tone?: NotificationTone,
  action?: { label: string; run: () => void | Promise<void> },
) => number;

export type ReportFailureFn = (
  title: string,
  error: unknown,
  visible?: boolean,
) => void;
