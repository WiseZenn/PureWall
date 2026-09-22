export type WorkspacePresentation = "loading" | "empty" | "gallery";

export function workspacePresentation(input: {
  isLoading: boolean;
  total: number;
}): WorkspacePresentation {
  if (input.total > 0) return "gallery";
  return input.isLoading ? "loading" : "empty";
}
