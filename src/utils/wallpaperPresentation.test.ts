import { describe, expect, it } from "vitest";
import type { WallpaperEntry } from "../stores/wallpapers";
import { wallpaperAccessibleLabel } from "./wallpaperPresentation";

function wallpaper(
  overrides: Partial<WallpaperEntry> = {},
): WallpaperEntry {
  return {
    id: 1,
    path: "D:\\Walls\\lake.jpg",
    hash: "hash",
    source: "folder",
    display_title: "",
    rating: 0,
    play_count: 0,
    last_played: null,
    created_at: "2026-07-30T00:00:00Z",
    blacklisted: false,
    width: 0,
    height: 0,
    file_size: 0,
    tags: [],
    ...overrides,
  };
}

describe("wallpaperAccessibleLabel", () => {
  it("prefers a user-defined display title", () => {
    expect(
      wallpaperAccessibleLabel(
        wallpaper({
          display_title: "Morning lake",
          rating: 1,
          width: 1920,
          height: 1080,
          tags: [{ id: 1, name: "Nature", color: "#00aa88" }],
        }),
      ),
    ).toBe("Morning lake, Liked, 1920x1080");
  });

  it("falls back to a tag and then a generic wallpaper name", () => {
    expect(
      wallpaperAccessibleLabel(
        wallpaper({ tags: [{ id: 1, name: "Nature", color: "#00aa88" }] }),
      ),
    ).toBe("Nature wallpaper, Unrated");
    expect(wallpaperAccessibleLabel(wallpaper())).toBe("Wallpaper, Unrated");
  });
});
