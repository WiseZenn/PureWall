import type { LibrarySourceStatus } from "../stores/wallpapers";

export function primarySourceAction(
  status: LibrarySourceStatus,
): "rescan" | "retry" {
  return status === "offline" || status === "error" ? "retry" : "rescan";
}

export function removeSourceCopy(affectedWallpapers: number) {
  return {
    keepMetadata:
      "Keep ratings, tags, collections, and titles. Exclusive wallpapers become unavailable and can recover when a covering folder is added again.",
    clearMetadata: `Clear metadata for ${affectedWallpapers.toLocaleString()} wallpapers that are exclusive to this source.`,
    safety: "Neither option deletes, moves, or recycles original image files.",
  };
}
