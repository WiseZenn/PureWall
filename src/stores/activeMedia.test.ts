import { describe, expect, it } from "vitest";
import {
  beginActiveMedia,
  chooseSpeculativePreviewPaths,
  createActiveMediaState,
  previewAvailable,
  previewFailed,
  previewLoaded,
  thumbnailAvailable,
} from "./activeMedia";

describe("active media state", () => {
  it("shows the thumbnail immediately and commits the matching preview only after load", () => {
    const initial = createActiveMediaState();
    const loading = beginActiveMedia(initial, {
      path: "D:/walls/current.jpg",
      thumbnailUrl: "asset://current-thumb.jpg",
      previewUrl: "asset://current-preview.jpg",
    });

    expect(loading.phase).toBe("thumbnail");
    expect(loading.displayUrl).toBe("asset://current-thumb.jpg");
    expect(loading.pendingPreviewUrl).toBe("asset://current-preview.jpg");

    const ready = previewLoaded(
      loading,
      loading.generation,
      "asset://current-preview.jpg",
    );

    expect(ready.phase).toBe("preview");
    expect(ready.displayUrl).toBe("asset://current-preview.jpg");
    expect(ready.pendingPreviewUrl).toBe("");
  });

  it("ignores a late preview event from the previous wallpaper", () => {
    const first = beginActiveMedia(createActiveMediaState(), {
      path: "D:/walls/first.jpg",
      thumbnailUrl: "asset://first-thumb.jpg",
      previewUrl: "asset://first-preview.jpg",
    });
    const second = beginActiveMedia(first, {
      path: "D:/walls/second.jpg",
      thumbnailUrl: "asset://second-thumb.jpg",
      previewUrl: "asset://second-preview.jpg",
    });

    const unchanged = previewLoaded(
      second,
      first.generation,
      "asset://first-preview.jpg",
    );

    expect(unchanged).toEqual(second);
  });

  it("keeps the thumbnail visible when the preview fails", () => {
    const loading = beginActiveMedia(createActiveMediaState(), {
      path: "D:/walls/current.jpg",
      thumbnailUrl: "asset://current-thumb.jpg",
      previewUrl: "asset://current-preview.jpg",
    });

    const failed = previewFailed(
      loading,
      loading.generation,
      "Preview generation failed",
    );

    expect(failed.phase).toBe("error");
    expect(failed.displayUrl).toBe("asset://current-thumb.jpg");
    expect(failed.error).toBe("Preview generation failed");
  });

  it("fills an empty stage when the active thumbnail arrives after selection", () => {
    const waiting = beginActiveMedia(createActiveMediaState(), {
      path: "D:/walls/current.jpg",
    });

    const visible = thumbnailAvailable(
      waiting,
      "D:/walls/current.jpg",
      "asset://current-thumb.jpg",
    );

    expect(visible.phase).toBe("thumbnail");
    expect(visible.displayUrl).toBe("asset://current-thumb.jpg");
    expect(visible.thumbnailUrl).toBe("asset://current-thumb.jpg");
    expect(visible.generation).toBe(waiting.generation);
  });

  it("does not queue an already committed preview again", () => {
    const loading = beginActiveMedia(createActiveMediaState(), {
      path: "D:/walls/current.jpg",
      thumbnailUrl: "asset://current-thumb.jpg",
      previewUrl: "asset://current-preview.jpg",
    });
    const ready = previewLoaded(
      loading,
      loading.generation,
      "asset://current-preview.jpg",
    );

    const unchanged = previewAvailable(
      ready,
      ready.path,
      "asset://current-preview.jpg",
    );

    expect(unchanged).toBe(ready);
    expect(unchanged.pendingPreviewUrl).toBe("");
  });

  it("chooses only the next two distinct loaded wallpapers for speculative warmup", () => {
    expect(
      chooseSpeculativePreviewPaths("D:/walls/b.jpg", [
        "D:/walls/a.jpg",
        "D:/walls/b.jpg",
        "D:/walls/c.jpg",
        "D:/walls/c.jpg",
        "D:/walls/d.jpg",
      ]),
    ).toEqual(["D:/walls/c.jpg", "D:/walls/d.jpg"]);

    expect(chooseSpeculativePreviewPaths("D:/walls/d.jpg", ["a", "b", "d"]))
      .toEqual(["a", "b"]);
  });
});
