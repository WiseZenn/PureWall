export type BatchRelationOperation = "assign" | "unassign";

export interface BatchRelationItem {
  id: number;
  name: string;
}

export interface BatchRelationOption {
  label: string;
  value: string;
  group?: string;
}

export interface BatchRelationAction {
  operation: BatchRelationOperation;
  id: number;
}

export function buildBatchRelationOptions(
  relations: BatchRelationItem[],
  placeholder: string,
): BatchRelationOption[] {
  return [
    { label: placeholder, value: "" },
    ...relations.map((relation) => ({
      label: relation.name,
      value: `assign:${relation.id}`,
      group: "Add",
    })),
    ...relations.map((relation) => ({
      label: relation.name,
      value: `unassign:${relation.id}`,
      group: "Remove",
    })),
  ];
}

export function parseBatchRelationValue(
  value: string | number,
): BatchRelationAction | null {
  if (typeof value !== "string") return null;
  const match = /^(assign|unassign):([1-9]\d*)$/.exec(value);
  if (!match) return null;

  const id = Number(match[2]);
  if (!Number.isSafeInteger(id)) return null;

  return {
    operation: match[1] as BatchRelationOperation,
    id,
  };
}
