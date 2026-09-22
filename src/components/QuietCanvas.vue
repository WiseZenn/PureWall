<script setup lang="ts">
import { computed } from "vue";
import { useWallpaperStore } from "../stores/wallpapers";
import { useWorkspaceMode } from "../composables/useWorkspaceMode";
import AppIcon from "./AppIcon.vue";

const store = useWallpaperStore();
const { setWorkspaceMode } = useWorkspaceMode();

const resolution = computed(() => {
  const metadata = store.activeMetadata;
  return metadata?.width && metadata?.height
    ? `${metadata.width} × ${metadata.height}`
    : "PureWall collection";
});

const pauseLabel = computed(() => (store.isPaused ? "Resume rotation" : "Pause rotation"));
</script>

<template>
  <section
    class="quiet-canvas"
    :style="{ backgroundImage: store.activePreviewUrl ? `url(${store.activePreviewUrl})` : undefined }"
  >
    <div class="quiet-canvas__veil"></div>

    <header class="quiet-canvas__header">
      <div class="quiet-canvas__brand">
        <span class="brand-mark"><AppIcon name="logo" /></span>
        <div>
          <strong>PureWall</strong>
          <span>Quiet Canvas</span>
        </div>
      </div>
      <button class="quiet-canvas__exit" type="button" @click="setWorkspaceMode('workbench')">
        <AppIcon name="exit-quiet" />
        <span>Return to workspace</span>
      </button>
    </header>

    <div class="quiet-canvas__content">
      <span class="quiet-canvas__eyebrow">Current atmosphere</span>
      <p>{{ resolution }} · selected for this moment</p>

      <div class="quiet-canvas__actions">
        <button
          type="button"
          :class="{ active: store.currentWallpaper?.rating === 1 }"
          :disabled="!store.currentWallpaperPath"
          @click="store.likeCurrentWallpaper"
        >
          <AppIcon name="heart" />
          <span>Like</span>
        </button>
        <button class="primary" type="button" @click="store.nextWallpaper">
          <AppIcon name="next" />
          <span>Next wallpaper</span>
        </button>
        <button type="button" :class="{ active: store.isPaused }" @click="store.togglePause">
          <AppIcon :name="store.isPaused ? 'play' : 'pause'" />
          <span>{{ pauseLabel }}</span>
        </button>
      </div>
    </div>

    <footer class="quiet-canvas__footer">
      <span>{{ store.stats.total.toLocaleString() }} wallpapers curated</span>
      <span>Press the workspace button to manage your library</span>
    </footer>
  </section>
</template>
