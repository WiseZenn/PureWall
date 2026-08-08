<script setup lang="ts">
import { computed } from "vue";
import { useWallpaperStore } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";

const store = useWallpaperStore();

const pauseLabel = computed(() => (store.isPaused ? "Resume" : "Pause"));
</script>

<template>
  <div class="wallpaper-controls">
    <button
      class="wallpaper-action primary active"
      type="button"
      data-responsive-required-control="next"
      aria-label="Next wallpaper"
      title="Next wallpaper"
      @click="store.nextWallpaper"
    >
      <AppIcon name="next" />
      <span>Next</span>
    </button>
    <button
      class="wallpaper-action"
      :class="{ active: store.currentWallpaper?.rating === 1 }"
      :disabled="!store.currentWallpaperPath"
      type="button"
      data-responsive-required-control="like"
      aria-label="Like current wallpaper"
      title="Like current wallpaper"
      @click="store.likeCurrentWallpaper"
    >
      <AppIcon name="heart" />
      <span>Like</span>
    </button>
    <button
      class="wallpaper-action"
      :class="{ active: store.currentWallpaper?.rating === -1 }"
      :disabled="!store.currentWallpaperPath"
      type="button"
      data-responsive-required-control="dislike"
      aria-label="Dislike current wallpaper"
      title="Dislike current wallpaper"
      @click="store.dislikeCurrentWallpaper"
    >
      <AppIcon name="dislike" />
      <span>Dislike</span>
    </button>
    <button
      class="wallpaper-action"
      :class="{ active: store.isPaused }"
      type="button"
      data-responsive-required-control="pause"
      :aria-label="pauseLabel"
      :title="pauseLabel"
      @click="store.togglePause"
    >
      <AppIcon :name="store.isPaused ? 'play' : 'pause'" />
      <span>{{ pauseLabel }}</span>
    </button>
  </div>
</template>
