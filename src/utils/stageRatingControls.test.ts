import { describe, expect, it } from "vitest";
import {
  applyStageRating,
  runExclusiveStageAction,
  stageRatingControl,
} from "./stageRatingControls";

describe("stageRatingControl", () => {
  it("maps neutral stage ratings to direct Like and Dislike commands", () => {
    expect(stageRatingControl(0, "like")).toEqual({
      command: "like",
      label: "Like displayed wallpaper",
      pressed: false,
    });
    expect(stageRatingControl(0, "dislike")).toEqual({
      command: "dislike",
      label: "Dislike displayed wallpaper",
      pressed: false,
    });
  });

  it("clears a rating when its active stage control is pressed again", () => {
    expect(stageRatingControl(1, "like")).toEqual({
      command: "reset",
      label: "Unlike displayed wallpaper",
      pressed: true,
    });
    expect(stageRatingControl(-1, "dislike")).toEqual({
      command: "reset",
      label: "Clear dislike from displayed wallpaper",
      pressed: true,
    });
  });

  it("switches directly when the opposite stage rating is pressed", () => {
    expect(stageRatingControl(-1, "like")).toMatchObject({
      command: "like",
      pressed: false,
    });
    expect(stageRatingControl(1, "dislike")).toMatchObject({
      command: "dislike",
      pressed: false,
    });
  });
});

describe("applyStageRating", () => {
  it("routes the captured displayed path to the exact rating command", async () => {
    const calls: string[] = [];
    const actions = {
      like: async (path: string) => calls.push(`like:${path}`),
      dislike: async (path: string) => calls.push(`dislike:${path}`),
      reset: async (path: string) => calls.push(`reset:${path}`),
    };

    await expect(
      applyStageRating({ path: "D:/walls/displayed.jpg", rating: 0 }, "like", actions),
    ).resolves.toBe("like");
    await expect(
      applyStageRating({ path: "D:/walls/displayed.jpg", rating: 1 }, "like", actions),
    ).resolves.toBe("reset");
    await expect(
      applyStageRating({ path: "D:/walls/displayed.jpg", rating: 1 }, "dislike", actions),
    ).resolves.toBe("dislike");

    expect(calls).toEqual([
      "like:D:/walls/displayed.jpg",
      "reset:D:/walls/displayed.jpg",
      "dislike:D:/walls/displayed.jpg",
    ]);
  });
});

describe("runExclusiveStageAction", () => {
  it("rejects a duplicate while one dock action owns the gate and releases afterward", async () => {
    const gate = { value: false };
    let finish!: () => void;
    const firstAction = new Promise<void>((resolve) => {
      finish = resolve;
    });
    let duplicateCalls = 0;

    const first = runExclusiveStageAction(gate, () => firstAction);
    expect(gate.value).toBe(true);
    await expect(
      runExclusiveStageAction(gate, async () => {
        duplicateCalls += 1;
      }),
    ).resolves.toBe(false);
    expect(duplicateCalls).toBe(0);

    finish();
    await expect(first).resolves.toBe(true);
    expect(gate.value).toBe(false);
    await expect(runExclusiveStageAction(gate, async () => {})).resolves.toBe(true);
  });

  it("releases the gate when an action fails", async () => {
    const gate = { value: false };
    await expect(
      runExclusiveStageAction(gate, async () => {
        throw new Error("expected failure");
      }),
    ).rejects.toThrow("expected failure");
    expect(gate.value).toBe(false);
  });
});
