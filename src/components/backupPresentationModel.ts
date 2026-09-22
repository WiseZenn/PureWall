import type {
  BackupImportPreview,
  BackupImportResult,
} from "../stores/wallpapers";

export interface BackupSummaryRow {
  label: string;
  value: string;
}

export interface BackupImportPresentation {
  committed: boolean;
  title: string;
  summary: string;
  details: string[];
  warnings: string[];
}

export function backupPreviewRows(
  preview: BackupImportPreview,
): BackupSummaryRow[] {
  return [
    { label: "Sources", value: String(preview.sourceCount) },
    { label: "Wallpapers", value: String(preview.wallpaperCount) },
    { label: "Tags", value: String(preview.tagCount) },
    { label: "Collections", value: String(preview.collectionCount) },
    { label: "Settings", value: String(preview.settingCount) },
    { label: "New metadata", value: String(preview.newWallpapers) },
    {
      label: "Existing metadata updated",
      value: String(preview.overwrittenWallpapers),
    },
    { label: "Missing paths", value: String(preview.missingPaths) },
  ];
}

export function missingPathMessage(count: number): string {
  if (count === 0) {
    return "All referenced wallpaper paths are currently available.";
  }
  if (count === 1) {
    return "1 wallpaper path is currently unavailable.";
  }
  return `${count} wallpaper paths are currently unavailable.`;
}

function counted(
  count: number,
  singular: string,
  plural: string,
): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

export function backupImportPresentation(
  result: BackupImportResult,
): BackupImportPresentation {
  const { merge } = result;

  return {
    committed: result.committed,
    title: "Backup metadata imported",
    summary: `Committed metadata: ${merge.addedWallpapers} added, ${merge.updatedWallpapers} updated.`,
    details: [
      counted(merge.addedSources, "source added", "sources added"),
      counted(merge.createdTags, "tag created", "tags created"),
      counted(
        merge.createdCollections,
        "collection created",
        "collections created",
      ),
      counted(merge.updatedSettings, "setting updated", "settings updated"),
    ],
    warnings: [...result.warnings],
  };
}
