import { createPinia, setActivePinia } from "pinia";
import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { describe, expect, it, vi } from "vitest";
import type { WallpaperEntry } from "../stores/wallpapers";
import { useWallpaperStore } from "../stores/wallpapers";
import CurrentWallpaperPanel from "./CurrentWallpaperPanel.vue";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("CurrentWallpaperPanel control hierarchy", () => {
  it("keeps playback controls dominant and moves low-frequency settings away", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const app = createSSRApp(CurrentWallpaperPanel);
    app.use(pinia);

    const html = await renderToString(app);

    expect(html).toContain("Next");
    expect(html).toContain("Like");
    expect(html).toContain("Dislike");
    expect(html).toContain("Pause");
    expect(html).toContain('aria-label="Rotation interval"');
    expect(html).not.toContain('aria-label="Display mode"');
    expect(html).not.toContain("Focus pause");
  });

  it("derives rating controls from the wallpaper displayed on the stage", async () => {
    const displayed: WallpaperEntry = {
      id: 1,
      path: "D:/walls/displayed.jpg",
      hash: "displayed",
      source: "folder",
      display_title: "Displayed",
      rating: 1,
      play_count: 2,
      last_played: null,
      created_at: "2026-08-23T00:00:00Z",
      blacklisted: false,
      width: 1920,
      height: 1080,
      file_size: 42,
      tags: [],
    };
    const current = { ...displayed, id: 2, path: "D:/walls/current.jpg", rating: 0 };
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useWallpaperStore();
    store.wallpapers = [displayed];
    store.activeWallpaperPath = displayed.path;
    store.currentWallpaperPath = current.path;
    store.currentWallpaper = current;
    const app = createSSRApp(CurrentWallpaperPanel);
    app.use(pinia);

    const html = await renderToString(app);

    expect(html).toContain('aria-label="Unlike displayed wallpaper"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain('aria-busy="false"');
    expect(html).not.toContain('aria-label="Like current wallpaper"');
  });
});
