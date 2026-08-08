export type ActiveMediaPhase = "idle" | "thumbnail" | "preview" | "error";

export interface ActiveMediaState {
  phase: ActiveMediaPhase;
  path: string;
  displayUrl: string;
  thumbnailUrl: string;
  pendingPreviewUrl: string;
  generation: number;
  error: string;
}

export interface BeginActiveMediaInput {
  path: string;
  thumbnailUrl?: string;
  previewUrl?: string;
}

export function createActiveMediaState(): ActiveMediaState {
  return {
    phase: "idle",
    path: "",
    displayUrl: "",
    thumbnailUrl: "",
    pendingPreviewUrl: "",
    generation: 0,
    error: "",
  };
}

export function beginActiveMedia(
  state: ActiveMediaState,
  input: BeginActiveMediaInput,
): ActiveMediaState {
  const thumbnailUrl = input.thumbnailUrl || "";
  return {
    phase: thumbnailUrl ? "thumbnail" : "idle",
    path: input.path,
    displayUrl: thumbnailUrl,
    thumbnailUrl,
    pendingPreviewUrl: input.previewUrl || "",
    generation: state.generation + 1,
    error: "",
  };
}

export function previewAvailable(
  state: ActiveMediaState,
  path: string,
  previewUrl: string,
): ActiveMediaState {
  if (
    path !== state.path ||
    !previewUrl ||
    previewUrl === state.displayUrl ||
    previewUrl === state.pendingPreviewUrl
  ) {
    return state;
  }
  return {
    ...state,
    pendingPreviewUrl: previewUrl,
    error: "",
  };
}

export function thumbnailAvailable(
  state: ActiveMediaState,
  path: string,
  thumbnailUrl: string,
): ActiveMediaState {
  if (
    path !== state.path ||
    !thumbnailUrl ||
    state.phase === "preview"
  ) {
    return state;
  }

  return {
    ...state,
    phase: state.phase === "error" ? "error" : "thumbnail",
    displayUrl: thumbnailUrl,
    thumbnailUrl,
  };
}

export function previewLoaded(
  state: ActiveMediaState,
  generation: number,
  previewUrl: string,
): ActiveMediaState {
  if (
    generation !== state.generation ||
    !previewUrl ||
    previewUrl !== state.pendingPreviewUrl
  ) {
    return state;
  }

  return {
    ...state,
    phase: "preview",
    displayUrl: previewUrl,
    pendingPreviewUrl: "",
    error: "",
  };
}

export function previewFailed(
  state: ActiveMediaState,
  generation: number,
  error: string,
): ActiveMediaState {
  if (generation !== state.generation) return state;
  return {
    ...state,
    phase: "error",
    pendingPreviewUrl: "",
    error,
  };
}

export interface ActiveWallpaperSelectionInput {
  activePath: string;
  currentPath: string;
  loadedPaths: string[];
}

export function chooseActiveWallpaperPath({
  activePath,
  currentPath,
  loadedPaths,
}: ActiveWallpaperSelectionInput): string {
  if (currentPath && activePath === currentPath) {
    return currentPath;
  }
  if (activePath && loadedPaths.includes(activePath)) {
    return activePath;
  }
  if (currentPath && loadedPaths.includes(currentPath)) {
    return currentPath;
  }
  return loadedPaths[0] || "";
}

export function chooseSpeculativePreviewPaths(
  activePath: string,
  loadedPaths: string[],
  limit = 2,
): string[] {
  if (limit <= 0) return [];

  const uniquePaths = Array.from(new Set(loadedPaths.filter(Boolean)));
  const activeIndex = uniquePaths.indexOf(activePath);
  const startIndex = activeIndex >= 0 ? activeIndex + 1 : 0;
  const ordered = [
    ...uniquePaths.slice(startIndex),
    ...uniquePaths.slice(0, startIndex),
  ];

  return ordered
    .filter((path) => path !== activePath)
    .slice(0, limit);
}
