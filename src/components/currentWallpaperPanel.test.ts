import { createPinia, setActivePinia } from "pinia";
import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { describe, expect, it, vi } from "vitest";
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
});
