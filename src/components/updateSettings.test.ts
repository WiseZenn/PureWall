import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { createPinia, setActivePinia } from "pinia";
import { describe, expect, it, vi } from "vitest";
import UpdateSettings from "./UpdateSettings.vue";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage = (_event: unknown) => undefined;
  },
}));

describe("UpdateSettings", () => {
  it("renders the explicit update action and Windows installer warning without checking on mount", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const app = createSSRApp(UpdateSettings);
    app.use(pinia);

    const html = await renderToString(app);

    expect(html).toContain("Application updates");
    expect(html).toContain("Check for updates");
    expect(html).toContain("Windows will close PureWall");
  });
});
