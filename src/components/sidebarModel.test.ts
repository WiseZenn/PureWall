import { describe, expect, it, vi } from "vitest";
import {
  createCollectionFromDraft,
  sidebarFilters,
} from "./sidebarModel";

describe("sidebar behavior model", () => {
  it("exposes the complete library navigation with the Disliked count", () => {
    const stats = {
      total: 12,
      liked: 4,
      disliked: 3,
      blacklisted: 2,
      total_plays: 20,
    };

    expect(sidebarFilters.map(({ key, label }) => [key, label])).toEqual([
      ["all", "Library"],
      ["liked", "Liked"],
      ["disliked", "Disliked"],
      ["blacklisted", "Hidden"],
      ["tags", "Tags"],
    ]);
    expect(
      sidebarFilters.find(({ key }) => key === "disliked")?.count(stats, 0),
    ).toBe(3);
  });

  it("creates a collection through the collection API and ignores blank drafts", async () => {
    const createCollection = vi.fn(async () => ({
      id: 7,
      name: "Weekend",
      color: "#5b8def",
      wallpaper_count: 0,
    }));
    const createTag = vi.fn();
    const creator = { createCollection, createTag };

    await expect(
      createCollectionFromDraft(creator, "  Weekend  ", "#5b8def"),
    ).resolves.toBe(true);
    expect(createCollection).toHaveBeenCalledOnce();
    expect(createCollection).toHaveBeenCalledWith("Weekend", "#5b8def");
    expect(createTag).not.toHaveBeenCalled();

    await expect(
      createCollectionFromDraft(creator, "   ", "#ff7a59"),
    ).resolves.toBe(false);
    expect(createCollection).toHaveBeenCalledOnce();
  });
});
