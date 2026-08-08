import { describe, expect, it } from "vitest";
import { primarySourceAction, removeSourceCopy } from "./librarySourceModel";

describe("library source presentation model", () => {
  it("uses retry only for offline and error sources", () => {
    expect(primarySourceAction("online")).toBe("rescan");
    expect(primarySourceAction("scanning")).toBe("rescan");
    expect(primarySourceAction("offline")).toBe("retry");
    expect(primarySourceAction("error")).toBe("retry");
  });

  it("states the clear impact and original-file safety invariant", () => {
    const copy = removeSourceCopy(7);
    expect(copy.keepMetadata).toContain("Keep ratings, tags, collections");
    expect(copy.clearMetadata).toContain("7");
    expect(copy.safety).toBe(
      "Neither option deletes, moves, or recycles original image files.",
    );
  });
});
