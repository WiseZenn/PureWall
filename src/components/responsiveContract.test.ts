import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

function source(relativePath: string): string {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function cssRule(css: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]+)\\}`));
  expect(match, `Missing CSS rule for ${selector}`).not.toBeNull();
  return match?.[1] ?? "";
}

describe("responsive interaction contract", () => {
  it("adds a compact-height layout without hiding playback actions", () => {
    const styles = source("../styles.css");
    const marker = "@media (max-height: 680px) and (min-width: 701px)";
    const compactStart = styles.indexOf(marker);
    const nextMedia = styles.indexOf("\n@media", compactStart + marker.length);
    const compactHeight =
      compactStart >= 0
        ? styles.slice(compactStart, nextMedia >= 0 ? nextMedia : undefined)
        : "";

    expect(compactStart).toBeGreaterThanOrEqual(0);
    expect(compactHeight).toContain(
      ".living-gallery-shell .gallery-workspace",
    );
    expect(compactHeight).toContain(".living-gallery-shell .side-rail");
    expect(compactHeight).toContain(".living-stage__dock");
    expect(compactHeight).not.toMatch(
      /\.wallpaper-action\s*\{[^}]*display:\s*none/,
    );

    const requiredControls = [
      ["import-folder", source("./TitleBar.vue")],
      ["next", source("./WallpaperControls.vue")],
      ["like", source("./WallpaperControls.vue")],
      ["dislike", source("./WallpaperControls.vue")],
      ["pause", source("./WallpaperControls.vue")],
    ] as const;

    for (const [control, component] of requiredControls) {
      expect(
        component,
        "Missing compact-layout contract for required control: " + control,
      ).toContain('data-responsive-required-control="' + control + '"');
      expect(compactHeight).not.toMatch(
        new RegExp(
          '\\[data-responsive-required-control="' +
            control +
            '"\\]\\s*\\{[^}]*display:\\s*none',
        ),
      );
    }
  });

  it("wraps user-visible source and backup paths", () => {
    const sources = source("./LibrarySourcesSettings.vue");
    const backup = source("./LibraryBackupSettings.vue");
    const sourcePathRule = cssRule(sources, ".source-row__identity strong");
    const backupPathRule = cssRule(backup, ".backup-dialog__path");

    expect(sourcePathRule).toContain("overflow-wrap: anywhere");
    expect(sourcePathRule).toContain("white-space: normal");
    expect(sourcePathRule).not.toContain("text-overflow: ellipsis");
    expect(backupPathRule).toContain("overflow-wrap: anywhere");
    expect(backupPathRule).toContain("white-space: normal");
    expect(backupPathRule).not.toContain("text-overflow: ellipsis");
  });
});
