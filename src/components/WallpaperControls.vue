<script setup lang="ts">
import { computed, ref } from "vue";
import { useWallpaperStore } from "../stores/wallpapers";
import {
  applyStageRating,
  runExclusiveStageAction,
  stageRatingControl,
  type StageRatingIntent,
} from "../utils/stageRatingControls";
import AppIcon from "./AppIcon.vue";

const store = useWallpaperStore();
const isBusy = ref(false);

const pauseLabel = computed(() => (store.isPaused ? "Resume" : "Pause"));
const displayedWallpaper = computed(() => store.activeWallpaper);
const likeControl = computed(() =>
  stageRatingControl(displayedWallpaper.value?.rating ?? 0, "like"),
);
const dislikeControl = computed(() =>
  stageRatingControl(displayedWallpaper.value?.rating ?? 0, "dislike"),
);

async function runPlayback(action: "next" | "toggle_pause") {
  await runExclusiveStageAction(isBusy, async () => {
    if (action === "next") await store.nextWallpaper();
    else await store.togglePause();
  });
}

async function rateDisplayedWallpaper(intent: StageRatingIntent) {
  const target = displayedWallpaper.value;
  if (!target) return;

  await runExclusiveStageAction(isBusy, async () => {
    await applyStageRating(target, intent, {
      like: (path) => store.like(path),
      dislike: (path) => store.dislike(path),
      reset: (path) => store.resetRating(path),
    });
  });
}
</script>

<template>
  <div class="wallpaper-controls" :aria-busy="isBusy">
    <button
      class="wallpaper-action primary active"
      type="button"
      data-responsive-required-control="next"
      aria-label="Next wallpaper"
      title="Next wallpaper"
      :disabled="isBusy"
      @click="runPlayback('next')"
    >
      <AppIcon name="next" />
      <span>Next</span>
    </button>
    <button
      class="wallpaper-action"
      :class="{ active: likeControl.pressed }"
      :disabled="!displayedWallpaper || isBusy"
      type="button"
      data-responsive-required-control="like"
      :aria-label="likeControl.label"
      :aria-pressed="likeControl.pressed"
      :title="likeControl.label"
      @click="rateDisplayedWallpaper('like')"
    >
      <AppIcon name="heart" />
      <span>{{ likeControl.pressed ? "Unlike" : "Like" }}</span>
    </button>
    <button
      class="wallpaper-action"
      :class="{ active: dislikeControl.pressed }"
      :disabled="!displayedWallpaper || isBusy"
      type="button"
      data-responsive-required-control="dislike"
      :aria-label="dislikeControl.label"
      :aria-pressed="dislikeControl.pressed"
      :title="dislikeControl.label"
      @click="rateDisplayedWallpaper('dislike')"
    >
      <AppIcon name="dislike" />
      <span>{{ dislikeControl.pressed ? "Clear" : "Dislike" }}</span>
    </button>
    <button
      class="wallpaper-action"
      :class="{ active: store.isPaused }"
      :disabled="isBusy"
      type="button"
      data-responsive-required-control="pause"
      :aria-label="pauseLabel"
      :aria-pressed="store.isPaused"
      :title="pauseLabel"
      @click="runPlayback('toggle_pause')"
    >
      <AppIcon :name="store.isPaused ? 'play' : 'pause'" />
      <span>{{ pauseLabel }}</span>
    </button>
  </div>
</template>
