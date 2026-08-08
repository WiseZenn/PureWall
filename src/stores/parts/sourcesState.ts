// Library-source + backup command helpers for the wallpaper store (src/stores/parts).
//
// Plain TS module — never a Pinia store and never importing ../wallpapers.
// The store passes a typed context object holding the refs/callbacks this
// module needs; the store keeps thin facade wrappers with identical names.

import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type {
  BackupClientSettings,
  BackupExportResult,
  BackupImportPreview,
  BackupImportResult,
  ImportResult,
  LibrarySourceMutation,
  LibrarySourceOperation,
  NotifyFn,
  RemoveLibrarySourceImpact,
  RemoveLibrarySourceMode,
  ReportFailureFn,
} from "./types";

export interface SourcesStateContext {
  librarySourceBusy: Map<string, LibrarySourceOperation>;
  librarySourceErrors: Map<string, string>;
  isBackupBusy: Ref<boolean>;
  backupError: Ref<string>;
  reportFailure: ReportFailureFn;
  notify: NotifyFn;
  loadLibrarySources: () => Promise<void>;
  loadWallpapers: () => Promise<void>;
  loadStats: () => Promise<void>;
  loadTags: () => Promise<void>;
  loadCollections: () => Promise<void>;
  loadDisplayMode: () => Promise<void>;
  loadFocusModeStatus: () => Promise<void>;
  loadPauseState: () => Promise<void>;
}

function commandMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  return error instanceof Error ? error.message : String(error);
}

async function runBackupOperation<T>(
  failureTitle: string,
  run: () => Promise<T>,
  ctx: SourcesStateContext,
): Promise<T> {
  if (ctx.isBackupBusy.value) {
    throw new Error("A backup operation is already in progress.");
  }

  ctx.isBackupBusy.value = true;
  ctx.backupError.value = "";
  try {
    return await run();
  } catch (error) {
    ctx.backupError.value = commandMessage(error);
    ctx.reportFailure(failureTitle, error);
    throw error;
  } finally {
    ctx.isBackupBusy.value = false;
  }
}

async function runLibrarySourceOperation<T>(
  path: string,
  operation: LibrarySourceOperation,
  run: () => Promise<T>,
  ctx: SourcesStateContext,
): Promise<T> {
  ctx.librarySourceBusy.set(path, operation);
  ctx.librarySourceErrors.delete(path);
  try {
    return await run();
  } catch (error) {
    ctx.librarySourceErrors.set(path, commandMessage(error));
    ctx.reportFailure(`Library source ${operation} failed`, error);
    throw error;
  } finally {
    ctx.librarySourceBusy.delete(path);
  }
}

async function refreshAfterLibrarySourceMutation(ctx: SourcesStateContext) {
  await Promise.all([
    ctx.loadLibrarySources(),
    ctx.loadWallpapers(),
    ctx.loadStats(),
  ]);
}

export async function rescanLibrarySource(path: string, ctx: SourcesStateContext) {
  return runLibrarySourceOperation(path, "rescan", async () => {
    const result = await invoke<ImportResult>(
      "rescan_library_source",
      { path },
    );
    await refreshAfterLibrarySourceMutation(ctx);
    return result;
  }, ctx);
}

export async function retryLibrarySource(path: string, ctx: SourcesStateContext) {
  return runLibrarySourceOperation(path, "retry", async () => {
    const result = await invoke<ImportResult>(
      "retry_library_source",
      { path },
    );
    await refreshAfterLibrarySourceMutation(ctx);
    return result;
  }, ctx);
}

export async function previewRemoveLibrarySource(path: string) {
  return invoke<RemoveLibrarySourceImpact>(
    "preview_remove_library_source",
    { path },
  );
}

export async function removeLibrarySource(
  path: string,
  mode: RemoveLibrarySourceMode,
  ctx: SourcesStateContext,
) {
  return runLibrarySourceOperation(path, "remove", async () => {
    const result = await invoke<LibrarySourceMutation>(
      "remove_library_source",
      {
        path,
        mode,
      },
    );
    await refreshAfterLibrarySourceMutation(ctx);
    if (result.watcher_warning) {
      ctx.notify(
        "Library source needs attention",
        result.watcher_warning,
        "info",
      );
    }
    return result;
  }, ctx);
}

export async function relocateLibrarySource(
  path: string,
  newPath: string,
  ctx: SourcesStateContext,
) {
  return runLibrarySourceOperation(path, "relocate", async () => {
    const result = await invoke<LibrarySourceMutation>(
      "relocate_library_source",
      {
        path,
        newPath,
      },
    );
    await refreshAfterLibrarySourceMutation(ctx);
    if (result.watcher_warning) {
      ctx.notify(
        "Library source needs attention",
        result.watcher_warning,
        "info",
      );
    }
    return result;
  }, ctx);
}

export async function exportLibraryBackup(
  destination: string,
  clientSettings: BackupClientSettings,
  ctx: SourcesStateContext,
): Promise<BackupExportResult> {
  return runBackupOperation("Backup export failed", async () => {
    const result = await invoke<BackupExportResult>(
      "export_library_backup",
      {
        destination,
        clientSettings,
      },
    );
    ctx.notify(
      "Backup exported",
      `Saved metadata backup to ${result.path}.`,
      "success",
    );
    return result;
  }, ctx);
}

export async function previewBackupImport(
  path: string,
  ctx: SourcesStateContext,
): Promise<BackupImportPreview> {
  return runBackupOperation("Backup preview failed", () =>
    invoke<BackupImportPreview>("preview_backup_import", { path }),
  ctx);
}

async function refreshAfterBackupImport(ctx: SourcesStateContext) {
  await Promise.all([
    ctx.loadLibrarySources(),
    ctx.loadWallpapers(),
    ctx.loadTags(),
    ctx.loadCollections(),
    ctx.loadStats(),
    ctx.loadDisplayMode(),
    ctx.loadFocusModeStatus(),
    ctx.loadPauseState(),
  ]);
}

export async function importLibraryBackup(
  path: string,
  expectedDigest: string,
  ctx: SourcesStateContext,
): Promise<BackupImportResult> {
  return runBackupOperation("Backup import failed", async () => {
    const result = await invoke<BackupImportResult>(
      "import_library_backup",
      { path, expectedDigest },
    );
    if (!result.committed) {
      throw new Error("PureWall did not commit the backup metadata.");
    }

    await refreshAfterBackupImport(ctx);
    ctx.notify(
      "Backup imported",
      "Backup metadata was committed to PureWall.",
      "success",
    );
    if (result.warnings.length > 0) {
      ctx.notify(
        "Backup imported with warnings",
        result.warnings.join("\n"),
        "info",
      );
    }
    return result;
  }, ctx);
}
