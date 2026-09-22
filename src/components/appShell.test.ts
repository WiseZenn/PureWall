import { createPinia, setActivePinia } from "pinia";
import { createSSRApp, h } from "vue";
import { renderToString } from "@vue/server-renderer";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useInspector } from "../composables/useInspector";
import { useWallpaperStore } from "../stores/wallpapers";
import AppShell from "./AppShell.vue";

function emptyComponent(name: string) {
  return {
    name,
    render: () => null,
  };
}

vi.mock("./EmptyState.vue", () => ({ default: emptyComponent("EmptyState") }));
vi.mock("./Gallery.vue", () => ({ default: emptyComponent("Gallery") }));
vi.mock("./NotificationCenter.vue", () => ({
  default: emptyComponent("NotificationCenter"),
}));
vi.mock("./QuietCanvas.vue", () => ({ default: emptyComponent("QuietCanvas") }));
vi.mock("./Sidebar.vue", () => ({ default: emptyComponent("Sidebar") }));
vi.mock("./StatusBar.vue", () => ({ default: emptyComponent("StatusBar") }));
vi.mock("./TitleBar.vue", () => ({ default: emptyComponent("TitleBar") }));
vi.mock("./InspectorPanel.vue", () => ({
  default: {
    name: "InspectorPanel",
    render: () => h("aside", { "data-testid": "inspector-panel" }),
  },
}));

describe("AppShell inspector visibility", () => {
  beforeEach(() => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useInspector().openInspector();
  });

  it("keeps system settings accessible when the library is empty", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useWallpaperStore();
    store.stats.total = 0;
    store.setWorkspaceSection("settings");

    const app = createSSRApp(AppShell);
    app.use(pinia);

    const html = await renderToString(app);

    expect(html).toMatch(/class="[^"]*\binspector-is-open\b[^"]*"/);
    expect(html).toContain(
      'data-testid="inspector-panel"',
    );
  });
});
