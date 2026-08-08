import { afterEach, describe, expect, it, vi } from "vitest";
import { useInspector } from "./useInspector";

function focusTarget(connected = true) {
  return {
    isConnected: connected,
    focus: vi.fn(),
  } as unknown as HTMLElement;
}

describe("useInspector focus restoration", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("restores focus to the connected element that opened the inspector", async () => {
    const opener = focusTarget();
    const fallback = focusTarget();
    vi.stubGlobal("document", {
      getElementById: vi.fn(() => fallback),
    });
    const { openInspector, closeInspector } = useInspector();

    openInspector(opener);
    await closeInspector();

    expect(opener.focus).toHaveBeenCalledOnce();
    expect(fallback.focus).not.toHaveBeenCalled();
  });

  it("uses the main workspace when the opener is no longer connected", async () => {
    const opener = focusTarget(false);
    const fallback = focusTarget();
    vi.stubGlobal("document", {
      getElementById: vi.fn(() => fallback),
    });
    const { openInspector, closeInspector } = useInspector();

    openInspector(opener);
    await closeInspector();

    expect(opener.focus).not.toHaveBeenCalled();
    expect(fallback.focus).toHaveBeenCalledOnce();
  });
});
