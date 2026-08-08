import { createPinia, setActivePinia } from "pinia";
import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { describe, expect, it, vi } from "vitest";
import type { WallpaperEntry } from "../stores/wallpapers";
import { useWallpaperStore } from "../stores/wallpapers";
import WallpaperGrid from "./WallpaperGrid.vue";

const layout = vi.hoisted(() => ({ width: 400 }));

vi.mock("@vueuse/core", async () => {
  const { computed, ref } = await import("vue");

  return {
    useElementSize: () => ({
      width: computed(() => layout.width),
      height: ref(600),
    }),
    useVirtualList: (rows: { value: WallpaperEntry[][] }) => ({
      list: computed(() =>
        rows.value.map((data, index) => ({ data, index })),
      ),
      scrollTo: () => undefined,
      containerProps: {
        ref: ref<HTMLElement | null>(null),
        onScroll: () => undefined,
        style: {},
      },
      wrapperProps: {},
    }),
  };
});

vi.mock("../utils/thumbnailRequestScheduler", () => ({
  createThumbnailRequestScheduler: () => ({
    schedule: () => undefined,
    cancel: () => undefined,
  }),
}));

function wallpaper(id: number): WallpaperEntry {
  return {
    id,
    path: "D:\\Walls\\wallpaper-" + id + ".jpg",
    hash: "hash-" + id,
    source: "folder",
    display_title: "Wallpaper " + id,
    rating: 0,
    play_count: 0,
    last_played: null,
    created_at: "2026-08-01T00:00:00Z",
    blacklisted: false,
    width: 1920,
    height: 1080,
    file_size: 1024,
    tags: [],
  };
}

async function renderGrid(total: number, loaded: number, width: number) {
  layout.width = width;
  const pinia = createPinia();
  setActivePinia(pinia);
  const store = useWallpaperStore();
  store.wallpapers = Array.from({ length: loaded }, (_, index) => wallpaper(index + 1));
  store.wallpaperTotal = total;
  store.isLoading = false;
  store.wallpaperViewMode = "grid";

  const app = createSSRApp(WallpaperGrid);
  app.use(pinia);
  return renderToString(app);
}

describe("WallpaperGrid row semantics", () => {
  it("reports total rows and rendered indexes for a paginated two-column grid", async () => {
    const html = await renderGrid(100, 3, 400);

    expect(html).toContain('role="grid"');
    expect(html).toContain('aria-rowcount="50"');
    expect(html.match(/role="row"/g)).toHaveLength(2);
    expect([...html.matchAll(/aria-rowindex="(\d+)"/g)].map((match) => match[1])).toEqual([
      "1",
      "2",
    ]);
  });

  it("reports zero rows for an empty result set", async () => {
    const html = await renderGrid(0, 0, 400);

    expect(html).toContain('aria-rowcount="0"');
  });

  it.each([
    ["three columns", 600, 34],
    ["the minimum one column", 0, 100],
  ] as const)("recomputes total rows for %s", async (_case, width, expectedRows) => {
    const html = await renderGrid(100, 3, width);

    expect(html).toContain('aria-rowcount="' + expectedRows + '"');
  });
});
