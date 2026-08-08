import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

function cssBlock(css: string, selector: string) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))?.[1] ?? "";
}

describe("Living Stage title contrast", () => {
  it("uses an image-safe light foreground independent of the app theme", () => {
    const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
    const titleRule = cssBlock(css, ".living-stage__copy h2");

    expect(titleRule).toContain("color: #fff;");
    expect(titleRule).toContain("text-shadow:");
  });

  it("allows an existing SQLite title to be explicitly retried without rewriting on every blur", () => {
    const component = readFileSync(new URL("../components/InspectorPanel.vue", import.meta.url), "utf8");
    expect(component).toContain("async function saveDisplayTitle(force = false)");
    expect(component).toContain("if (!force && nextTitle === wallpaper.display_title) return;");
    expect(component).toContain('@blur="() => saveDisplayTitle()"');
    expect(component).toContain('@keydown.enter.prevent="saveDisplayTitle(true)"');
  });

  it("patches the bootstrapped active wallpaper when it is outside the loaded page", () => {
    const store = readFileSync(new URL("../stores/wallpapers.ts", import.meta.url), "utf8");

    expect(store).toContain("if (bootstrappedWallpaper.value?.path === path) {");
    expect(store).toContain(
      "bootstrappedWallpaper.value = { ...bootstrappedWallpaper.value, ...patch };",
    );
  });
});
