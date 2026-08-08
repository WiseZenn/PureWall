<script setup lang="ts">
import { onMounted } from "vue";
import { useWallpaperStore } from "../stores/wallpapers";
import AppShell from "../components/AppShell.vue";

const store = useWallpaperStore();

onMounted(async () => {
  document.body.classList.remove("widget-view");
  await store.setupListeners();
  await store.bootstrapActiveWallpaper();
  // Library, metadata, stats, and runtime state are independent queries —
  // run them concurrently to reduce startup latency.
  await Promise.all([
    store.loadTags(),
    store.loadCollections(),
    store.loadWallpapers(),
    store.loadStats(),
    store.loadPhaseFour(),
  ]);
});
</script>

<template>
  <AppShell />
</template>
