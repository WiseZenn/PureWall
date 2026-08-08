import { describe, expect, it, vi } from "vitest";
import { createThumbnailRequestScheduler } from "./thumbnailRequestScheduler";

describe("createThumbnailRequestScheduler", () => {
  it("requests every path that is still visible after a debounced reschedule", async () => {
    vi.useFakeTimers();
    const request = vi.fn<(paths: string[]) => void>();
    const scheduler = createThumbnailRequestScheduler(request, 80);

    scheduler.schedule(["first.jpg", "second.jpg"]);
    scheduler.schedule(["first.jpg", "second.jpg", "third.jpg"]);

    await vi.advanceTimersByTimeAsync(80);

    expect(request).toHaveBeenCalledTimes(1);
    expect(request).toHaveBeenCalledWith([
      "first.jpg",
      "second.jpg",
      "third.jpg",
    ]);
    vi.useRealTimers();
  });
});
