import { describe, expect, it } from "vitest";
import {
  buildBatchRelationOptions,
  parseBatchRelationValue,
} from "./batchRelationMenuModel";

describe("batch relation menu model", () => {
  it("builds grouped Add and Remove actions for every relation", () => {
    const options = buildBatchRelationOptions(
      [
        { id: 3, name: "Blue" },
        { id: 7, name: "Quiet" },
      ],
      "Tags",
    );

    expect(options).toEqual([
      { label: "Tags", value: "" },
      {
        label: "Blue",
        value: "assign:3",
        group: "Add",
      },
      {
        label: "Quiet",
        value: "assign:7",
        group: "Add",
      },
      {
        label: "Blue",
        value: "unassign:3",
        group: "Remove",
      },
      {
        label: "Quiet",
        value: "unassign:7",
        group: "Remove",
      },
    ]);
  });

  it("parses supported relation actions", () => {
    expect(parseBatchRelationValue("assign:7")).toEqual({
      operation: "assign",
      id: 7,
    });
    expect(parseBatchRelationValue("unassign:11")).toEqual({
      operation: "unassign",
      id: 11,
    });
  });

  it("rejects placeholders and malformed values", () => {
    expect(parseBatchRelationValue("")).toBeNull();
    expect(parseBatchRelationValue("remove:7")).toBeNull();
    expect(parseBatchRelationValue("assign:0")).toBeNull();
    expect(parseBatchRelationValue("assign:-2")).toBeNull();
    expect(parseBatchRelationValue("assign:not-a-number")).toBeNull();
    expect(parseBatchRelationValue(7)).toBeNull();
  });
});
