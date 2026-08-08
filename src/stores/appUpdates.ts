import { ref } from "vue";
import { defineStore } from "pinia";
import { Channel, invoke } from "@tauri-apps/api/core";
import packageJson from "../../package.json";

export interface UpdateMetadata {
  version: string;
  currentVersion: string;
  date: string | null;
  body: string | null;
}

export interface DownloadProgress {
  downloaded: number;
  contentLength: number | null;
}

export type AppUpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "current"; currentVersion: string }
  | { status: "available"; update: UpdateMetadata }
  | { status: "downloading"; update: UpdateMetadata; progress: DownloadProgress }
  | { status: "readyToRestart"; update: UpdateMetadata }
  | { status: "completed"; update: UpdateMetadata }
  | { status: "error"; message: string; retry: "check" | "install" };

interface DownloadEvent {
  event: "started" | "progress" | "finished";
  data?: {
    contentLength?: number | null;
    downloaded?: number;
  };
}

const CURRENT_VERSION = packageJson.version;

function errorMessage(error: unknown, fallback: string): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  if (error instanceof Error && error.message.trim()) return error.message;
  return fallback;
}

export const useAppUpdatesStore = defineStore("appUpdates", () => {
  const state = ref<AppUpdateState>({ status: "idle" });
  let activeOperation: "check" | "install" | null = null;
  let pendingUpdate: UpdateMetadata | null = null;

  async function checkForUpdates() {
    if (activeOperation) return;
    activeOperation = "check";
    state.value = { status: "checking" };
    try {
      const update = await invoke<UpdateMetadata | null>("fetch_update");
      pendingUpdate = update;
      state.value = update
        ? { status: "available", update }
        : { status: "current", currentVersion: CURRENT_VERSION };
    } catch (error) {
      state.value = {
        status: "error",
        message: errorMessage(error, "Could not check for updates. Please try again."),
        retry: "check",
      };
    } finally {
      activeOperation = null;
    }
  }

  async function installUpdate() {
    if (activeOperation || !pendingUpdate) return;
    const update = pendingUpdate;
    activeOperation = "install";
    state.value = {
      status: "downloading",
      update,
      progress: { downloaded: 0, contentLength: null },
    };

    const channel = new Channel<DownloadEvent>();
    channel.onmessage = (event) => {
      if (event.event === "started") {
        state.value = {
          status: "downloading",
          update,
          progress: {
            downloaded: 0,
            contentLength: event.data?.contentLength ?? null,
          },
        };
      } else if (event.event === "progress") {
        state.value = {
          status: "downloading",
          update,
          progress: {
            downloaded: event.data?.downloaded ?? 0,
            contentLength: event.data?.contentLength ?? null,
          },
        };
      }
    };

    try {
      await invoke("install_update", { onEvent: channel });
      state.value = { status: "readyToRestart", update };
      pendingUpdate = null;
    } catch (error) {
      state.value = {
        status: "error",
        message: errorMessage(error, "Could not install the update. Please try again."),
        retry: "install",
      };
    } finally {
      activeOperation = null;
    }
  }

  async function retry() {
    if (state.value.status !== "error" || activeOperation) return;
    if (state.value.retry === "check") {
      await checkForUpdates();
    } else {
      await installUpdate();
    }
  }

  return {
    state,
    currentVersion: CURRENT_VERSION,
    checkForUpdates,
    installUpdate,
    retry,
  };
});
