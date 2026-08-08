<script setup lang="ts">
import { ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useWorkspaceMode } from "../composables/useWorkspaceMode";
import { useInspector } from "../composables/useInspector";
import { useWallpaperStore } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";
import SearchToolbar from "./SearchToolbar.vue";

const appWindow = getCurrentWindow();
const { setWorkspaceMode } = useWorkspaceMode();
const { toggleInspector } = useInspector();
const store = useWallpaperStore();
const importStatus = ref("");
const imageFilters = [{ name: "Images", extensions: ["jpg", "jpeg", "png", "bmp", "webp"] }];

function showImportStatus(message: string) {
  importStatus.value = message;
  window.setTimeout(() => {
    if (importStatus.value === message) importStatus.value = "";
  }, 3000);
}
function toggleInspectorFromButton(event: MouseEvent) {
  toggleInspector(event.currentTarget as HTMLElement);
}


async function importFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    try {
      await store.importFolder(selected);
      // Import runs on a background thread now — the store shows a
      // notification via the `import-complete` event when finished.
      showImportStatus("Importing…");
    } catch {
      // The store reports the visible error notification.
    }
  }
}

async function importImages() {
  const selected = await open({ directory: false, multiple: true, filters: imageFilters });
  const paths = Array.isArray(selected) ? selected : typeof selected === "string" ? [selected] : [];
  if (paths.length > 0) {
    try {
      const result = await store.importFiles(paths);
      showImportStatus(`Imported ${result.imported}`);
    } catch {
      // The store reports the visible error notification.
    }
  }
}

async function minimize() {
  try {
    await appWindow.minimize();
  } catch (error) {
    console.error("Failed to minimize window:", error);
  }
}

async function maximize() {
  try {
    if (await appWindow.isMaximized()) await appWindow.unmaximize();
    else await appWindow.maximize();
  } catch (error) {
    console.error("Failed to toggle maximize:", error);
  }
}

async function close() {
  try {
    await appWindow.hide();
  } catch (error) {
    console.error("Failed to hide window:", error);
  }
}
</script>

<template>
  <header class="titlebar-shell">
    <div class="titlebar-brand">
      <span class="brand-mark" aria-hidden="true">
        <AppIcon name="logo" />
      </span>
      <span class="brand-lockup">
        <span class="brand-name">PURE WALL</span>
        <span class="brand-subtitle">Living Gallery</span>
      </span>
    </div>

    <SearchToolbar />

    <div class="titlebar-primary-actions">
      <span v-if="importStatus" class="titlebar-import-status">{{ importStatus }}</span>
      <button
        class="titlebar-import primary"
        type="button"
        data-responsive-required-control="import-folder"
        :disabled="store.isImporting"
        @click="importFolder"
      >
        <AppIcon name="folder" />
        <span>Import folder</span>
      </button>
      <button class="titlebar-import" type="button" :disabled="store.isImporting" @click="importImages">
        <AppIcon name="image" />
        <span>Add images</span>
      </button>
      <button class="window-button mode-button" type="button" aria-label="Toggle inspector" title="Toggle inspector" @click="toggleInspectorFromButton">
        <AppIcon name="panel-right" />
      </button>
    </div>

    <div class="titlebar-window-controls">
      <button
        class="window-button mode-button"
        type="button"
        aria-label="Enter Quiet Canvas"
        title="Enter Quiet Canvas"
        @click="setWorkspaceMode('quiet')"
      >
        <AppIcon name="quiet-canvas" />
      </button>
      <button class="window-button" type="button" aria-label="Minimize" title="Minimize" @click="minimize">
        <span class="minimize-glyph" aria-hidden="true"></span>
      </button>
      <button class="window-button" type="button" aria-label="Maximize" title="Maximize" @click="maximize">
        <span class="maximize-glyph" aria-hidden="true"></span>
      </button>
      <button class="window-button danger" type="button" aria-label="Close" title="Close" @click="close">
        <AppIcon name="x" />
      </button>
    </div>
  </header>
</template>
