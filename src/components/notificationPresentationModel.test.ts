import { describe, expect, it } from "vitest";
import { notificationSemantics } from "./notificationPresentationModel";

describe("notificationSemantics", () => {
  it("announces errors assertively", () => {
    expect(notificationSemantics("error")).toEqual({
      role: "alert",
      live: "assertive",
    });
  });

  it.each(["info", "success"] as const)(
    "announces %s messages politely",
    (tone) => {
      expect(notificationSemantics(tone)).toEqual({
        role: "status",
        live: "polite",
      });
    },
  );
});
