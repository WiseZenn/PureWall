<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useWallpaperStore } from "../stores/wallpapers";
import CompactDropdown from "./CompactDropdown.vue";
import WallpaperControls from "./WallpaperControls.vue";
import { wallpaperAccessibleLabel } from "../utils/wallpaperPresentation";

const store = useWallpaperStore();
const currentInterval = ref(600);

const intervals = [
  { label: "1 minute", value: 60 },
  { label: "5 minutes", value: 300 },
  { label: "10 minutes", value: 600 },
  { label: "30 minutes", value: 1800 },
];

const displayTitle = computed(() => store.activeWallpaper?.display_title.trim() ?? "");
const resolution = computed(() => {
  const metadata = store.activeMetadata;
  return metadata && metadata.width > 0 ? `${metadata.width} × ${metadata.height}` : "Ready to play";
});
const stageTags = computed(() => store.activeWallpaper?.tags.slice(0, 3) ?? []);
const extraTagCount = computed(() => Math.max(0, (store.activeWallpaper?.tags.length ?? 0) - stageTags.value.length));

onMounted(async () => {
  try {
    currentInterval.value = await invoke("get_rotation_interval");
  } catch {}
});

async function setRotationInterval(value: string | number) {
  const seconds = Number(value);
  if (!seconds) return;
  const previous = currentInterval.value;
  currentInterval.value = seconds;
  try {
    await invoke("set_rotation_interval", { seconds });
  } catch (error) {
    // The backend rejected the value: restore the last known-good interval so
    // the UI never displays a setting the Rust side refused.
    currentInterval.value = previous;
    console.warn("[PureWall] Failed to set rotation interval:", error);
  }
}

</script>

<template>
  <section class="current-wallpaper-panel living-stage">
    <img
      v-if="store.activePreviewUrl && store.activeWallpaper"
      class="living-stage__image"
      :src="store.activePreviewUrl"
      :alt="wallpaperAccessibleLabel(store.activeWallpaper)"
      @error="() => console.warn('[PureWall] Stage preview load failed:', store.activeWallpaper?.path, store.activePreviewUrl)"
    />
    <div v-else class="living-stage__placeholder">
      <div class="spinner"></div>
      <span>Loading preview</span>
    </div>
    <p
      v-if="store.activeMediaState.phase === 'error'"
      class="living-stage__preview-status"
      role="status"
    >
      High-resolution preview unavailable
    </p>
    <div v-if="displayTitle" class="living-stage__copy">
      <h2>{{ displayTitle }}</h2>
      <div class="living-stage__meta">
        <span>{{ resolution }}</span>
        <span>{{ store.activeWallpaper?.play_count || 0 }} plays</span>
        <span v-for="tag in stageTags" :key="tag.id" class="living-stage__tag">
          <i :style="{ backgroundColor: tag.color }"></i>
          {{ tag.name }}
        </span>
        <span v-if="extraTagCount > 0">+{{ extraTagCount }}</span>
      </div>
    </div>

    <div class="living-stage__dock">
      <WallpaperControls />
      <div class="stage-quick-settings">
        <CompactDropdown
          icon="calendar"
          label="Rotation interval"
          :model-value="currentInterval"
          :options="intervals"
          @change="setRotationInterval"
        />
      </div>
    </div>
  </section>
</template>
