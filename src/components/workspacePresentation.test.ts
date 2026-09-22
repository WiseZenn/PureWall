import { describe, expect, it } from "vitest";
import { workspacePresentation } from "./workspacePresentation";

describe("workspacePresentation", () => {
  it("keeps first bootstrap loading distinct from an empty library", () => {
    expect(workspacePresentation({ isLoading: true, total: 0 })).toBe("loading");
    expect(workspacePresentation({ isLoading: false, total: 0 })).toBe("empty");
  });

  it("keeps an existing library visible during later reloads", () => {
    expect(workspacePresentation({ isLoading: true, total: 4 })).toBe("gallery");
    expect(workspacePresentation({ isLoading: false, total: 4 })).toBe("gallery");
  });
});
