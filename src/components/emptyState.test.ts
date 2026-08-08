import { createPinia, setActivePinia } from "pinia";
import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useWallpaperStore } from "../stores/wallpapers";
import EmptyState from "./EmptyState.vue";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

describe("EmptyState", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  async function renderEmptyState() {
    const pinia = createPinia();
    setActivePinia(pinia);
    const app = createSSRApp(EmptyState);
    app.use(pinia);
    return {
      html: await renderToString(app),
      store: useWallpaperStore(),
    };
  }

  it("leads with one folder action and a concise local-only statement", async () => {
    const { html } = await renderEmptyState();

    expect(html).toContain("Choose wallpaper folder");
    expect(html).toContain("Your wallpapers stay on this device");
    expect(html).toContain("PureWall does not upload your images");
    expect(html).toMatch(/class="empty-cta primary"/);
    expect(html).toMatch(/class="empty-cta secondary"/);
  });

  it("exposes a busy state and disables competing imports", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useWallpaperStore();
    store.isImporting = true;
    const app = createSSRApp(EmptyState);
    app.use(pinia);

    const html = await renderToString(app);

    expect(html).toContain('aria-busy="true"');
    expect(html.match(/disabled/g)?.length).toBe(2);
    expect(html).toContain("Importing");
  });
});
