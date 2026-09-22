<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useWallpaperStore } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";

const store = useWallpaperStore();

async function chooseFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    await store.setFolder(selected);
  }
}

async function chooseImages() {
  const selected = await open({
    directory: false,
    multiple: true,
    filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "bmp", "webp"] }],
  });
  const paths = Array.isArray(selected) ? selected : typeof selected === "string" ? [selected] : [];
  if (paths.length > 0) {
    try {
      await store.importFiles(paths);
    } catch {
      // The store reports the visible error notification.
    }
  }
}
</script>

<template>
  <section class="empty-onboarding" :aria-busy="store.isImporting" aria-live="polite">
    <div class="empty-mark">
      <AppIcon name="logo" />
    </div>
    <div class="empty-copy">
      <h2>Choose your wallpaper folder</h2>
      <p>Select a local folder to build your library and start playback.</p>
      <p class="empty-trust">Your wallpapers stay on this device. PureWall does not upload your images.</p>
    </div>
    <div class="empty-actions">
      <button
        class="empty-cta primary"
        type="button"
        :disabled="store.isImporting"
        @click="chooseFolder"
      >
        <AppIcon name="folder" />
        <span>{{ store.isImporting ? "Importing…" : "Choose wallpaper folder" }}</span>
      </button>
      <button class="empty-cta secondary" type="button" :disabled="store.isImporting" @click="chooseImages">
        <AppIcon name="image" />
        <span>Add individual images</span>
      </button>
    </div>
  </section>
</template>
