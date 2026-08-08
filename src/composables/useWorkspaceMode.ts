import { readonly, ref } from "vue";

export type WorkspaceMode = "workbench" | "quiet";

const STORAGE_KEY = "purewall-workspace-mode";
const workspaceMode = ref<WorkspaceMode>("workbench");

function applyWorkspaceMode(mode: WorkspaceMode) {
  workspaceMode.value = mode;
  document.documentElement.dataset.workspaceMode = mode;
}

export function initializeWorkspaceMode() {
  let savedMode: string | null = null;

  try {
    savedMode = window.localStorage.getItem(STORAGE_KEY);
  } catch {
    // Mode switching still works for the current session when storage is unavailable.
  }

  applyWorkspaceMode(savedMode === "quiet" ? "quiet" : "workbench");
}

export function useWorkspaceMode() {
  function setWorkspaceMode(mode: WorkspaceMode) {
    applyWorkspaceMode(mode);

    try {
      window.localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      // Keep the selected mode even when storage is unavailable.
    }
  }

  function toggleWorkspaceMode() {
    setWorkspaceMode(workspaceMode.value === "quiet" ? "workbench" : "quiet");
  }

  return {
    workspaceMode: readonly(workspaceMode),
    setWorkspaceMode,
    toggleWorkspaceMode,
  };
}
