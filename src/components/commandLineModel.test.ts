import { describe, expect, it } from "vitest";
import {
  COMMAND_LINE_SUBTITLE,
  COMMAND_LINE_TITLE,
  commandLineRows,
} from "./commandLineModel";

describe("command-line presentation model", () => {
  it("labels CLI actions accurately without presenting them as keyboard shortcuts", () => {
    expect(COMMAND_LINE_TITLE).toBe("Command Line");
    expect(COMMAND_LINE_SUBTITLE).toBe("Local CLI actions");
    expect(commandLineRows).toEqual([
      { label: "PW: Next", value: "--action next" },
      { label: "PW: Like", value: "--action like" },
      { label: "PW: Dislike", value: "--action dislike" },
      { label: "PW: Pause", value: "--action pause" },
    ]);
  });
});
