import { describe, expect, it } from "vitest";
import { chooseActiveWallpaperPath } from "./activeMedia";

describe("startup active wallpaper selection", () => {
  it("keeps the bootstrapped current wallpaper when it is outside the first page", () => {
    const selected = chooseActiveWallpaperPath({
      activePath: "D:/walls/current.jpg",
      currentPath: "D:/walls/current.jpg",
      loadedPaths: ["D:/walls/first.jpg", "D:/walls/second.jpg"],
    });

    expect(selected).toBe("D:/walls/current.jpg");
  });

  it("uses the first loaded wallpaper when no persisted current wallpaper exists", () => {
    const selected = chooseActiveWallpaperPath({
      activePath: "",
      currentPath: "",
      loadedPaths: ["D:/walls/first.jpg", "D:/walls/second.jpg"],
    });

    expect(selected).toBe("D:/walls/first.jpg");
  });
});
