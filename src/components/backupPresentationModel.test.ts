import { describe, expect, it } from "vitest";
import type {
  BackupImportPreview,
  BackupImportResult,
} from "../stores/wallpapers";
import {
  backupImportPresentation,
  backupPreviewRows,
  missingPathMessage,
} from "./backupPresentationModel";

const preview: BackupImportPreview = {
  contentDigest: "sha256:preview-fixture",
  sourceCount: 2,
  wallpaperCount: 14,
  tagCount: 5,
  collectionCount: 3,
  settingCount: 4,
  newWallpapers: 9,
  overwrittenWallpapers: 5,
  missingPaths: 2,
  warnings: ["2 wallpaper paths are currently unavailable."],
};

const result: BackupImportResult = {
  committed: true,
  merge: {
    addedWallpapers: 9,
    updatedWallpapers: 5,
    addedSources: 1,
    createdTags: 2,
    createdCollections: 1,
    updatedSettings: 4,
    clientSettings: {
      theme: "dark",
      workspaceMode: "quiet",
    },
  },
  warnings: [
    "2 wallpaper paths remain unavailable.",
    "D:\\Offline could not start a watcher.",
  ],
};

describe("backup presentation model", () => {
  it("maps every preview count to a stable summary row", () => {
    expect(backupPreviewRows(preview)).toEqual([
      { label: "Sources", value: "2" },
      { label: "Wallpapers", value: "14" },
      { label: "Tags", value: "5" },
      { label: "Collections", value: "3" },
      { label: "Settings", value: "4" },
      { label: "New metadata", value: "9" },
      { label: "Existing metadata updated", value: "5" },
      { label: "Missing paths", value: "2" },
    ]);
  });

  it("uses precise missing-path copy for zero, singular, and plural counts", () => {
    expect(missingPathMessage(0)).toBe(
      "All referenced wallpaper paths are currently available.",
    );
    expect(missingPathMessage(1)).toBe(
      "1 wallpaper path is currently unavailable.",
    );
    expect(missingPathMessage(2)).toBe(
      "2 wallpaper paths are currently unavailable.",
    );
  });

  it("describes committed metadata counts without claiming image restoration", () => {
    const presentation = backupImportPresentation(result);

    expect(presentation.title).toBe("Backup metadata imported");
    expect(presentation.summary).toBe(
      "Committed metadata: 9 added, 5 updated.",
    );
    expect(presentation.details).toEqual([
      "1 source added",
      "2 tags created",
      "1 collection created",
      "4 settings updated",
    ]);
    expect(
      [presentation.title, presentation.summary, ...presentation.details]
        .join(" ")
        .toLowerCase(),
    ).not.toContain("image");
    expect(presentation.warnings).toEqual(result.warnings);
  });

  it("keeps committed success and post-commit warnings visible together", () => {
    const presentation = backupImportPresentation(result);

    expect(presentation.committed).toBe(true);
    expect(presentation.warnings).toHaveLength(2);
  });
});
