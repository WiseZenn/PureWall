import { createPinia, setActivePinia } from "pinia";
import { readFileSync } from "node:fs";
import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { describe, expect, it } from "vitest";
import type { WallpaperEntry } from "../stores/wallpapers";
import { useWallpaperStore } from "../stores/wallpapers";
import InspectorPanel from "./InspectorPanel.vue";
import WallpaperGrid from "./WallpaperGrid.vue";
import WallpaperCard from "./WallpaperCard.vue";

const wallpaper: WallpaperEntry = {
  id: 1,
  path: "D:\\Walls\\lake.jpg",
  hash: "hash",
  source: "folder",
  display_title: "Morning lake",
  rating: 1,
  play_count: 4,
  last_played: null,
  created_at: "2026-07-30T00:00:00Z",
  blacklisted: false,
  width: 1920,
  height: 1080,
  file_size: 1024,
  tags: [],
};

function createStore() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return { pinia, store: useWallpaperStore() };
}

describe("semantic primary surfaces", () => {
  it("gives system inspectors a focus target and semantic heading", async () => {
    const { pinia, store } = createStore();
    store.setWorkspaceSection("settings");
    const app = createSSRApp(InspectorPanel);
    app.use(pinia);

    const html = await renderToString(app);

    expect(html).toContain('tabindex="-1"');
    expect(html).toContain('aria-labelledby="system-panel-settings-title"');
    expect(html).toContain(
      '<h2 id="system-panel-settings-title" class="system-panel__title">Settings</h2>',
    );
  });

  it("exposes the virtual wallpaper collection as a named grid", async () => {
    const { pinia, store } = createStore();
    store.wallpapers = [wallpaper];
    store.wallpaperTotal = 1;
    store.isLoading = false;
    const gridApp = createSSRApp(WallpaperGrid);
    gridApp.use(pinia);

    const gridHtml = await renderToString(gridApp);

    expect(gridHtml).toContain('role="grid"');
    expect(gridHtml).toContain('aria-label="Wallpaper library"');

    const cardApp = createSSRApp(WallpaperCard, {
      wallpaper,
      cardTabIndex: 0,
      viewMode: "grid",
    });
    cardApp.use(pinia);
    const cardHtml = await renderToString(cardApp);

    expect(cardHtml).toContain('role="gridcell"');
    expect(cardHtml).toContain('aria-label="Morning lake, Liked, 1920x1080"');

    const gridSource = readFileSync(
      new URL("./WallpaperGrid.vue", import.meta.url),
      "utf8",
    );
    expect(gridSource).toContain('role="row"');
  });
});
