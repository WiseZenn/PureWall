import { describe, expect, it } from "vitest";
import { nextThumbnailRetry } from "./thumbnailRetryPolicy";

describe("nextThumbnailRetry", () => {
  it("allows two exponential-backoff retries before reaching a terminal error", () => {
    expect(nextThumbnailRetry(0)).toEqual({ attempt: 1, delayMs: 250 });
    expect(nextThumbnailRetry(1)).toEqual({ attempt: 2, delayMs: 500 });
    expect(nextThumbnailRetry(2)).toBeNull();
    expect(nextThumbnailRetry(3)).toBeNull();
  });
});