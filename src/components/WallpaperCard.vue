<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import type { WallpaperEntry, WallpaperViewMode } from "../stores/wallpapers";
import { useWallpaperStore } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";
import IconButton from "./IconButton.vue";
import { useInspector } from "../composables/useInspector";
import { wallpaperAccessibleLabel } from "../utils/wallpaperPresentation";

const props = defineProps<{
  wallpaper: WallpaperEntry;
  viewMode?: WallpaperViewMode;
  cardTabIndex?: number;
}>();

const emit = defineEmits<{
  focusCard: [id: number];
  moveFocus: [event: KeyboardEvent, id: number];
}>();

const store = useWallpaperStore();
const { openInspector } = useInspector();
const isHovered = ref(false);
const showConfirmDelete = ref(false);
let deleteConfirmTimeout: number | undefined;

const thumbnailUrl = computed(() => store.thumbnails.get(props.wallpaper.path) || "");
const thumbnailError = computed(() => store.thumbnailErrors.get(props.wallpaper.path) || "");
const selected = computed(() => store.selectedPaths.has(props.wallpaper.path));
const active = computed(() => store.activeWallpaperPath === props.wallpaper.path);
const visibleTags = computed(() => props.wallpaper.tags.slice(0, 2));
const extraTagCount = computed(() => Math.max(0, props.wallpaper.tags.length - visibleTags.value.length));
const isListView = computed(() => props.viewMode === "list");

function onCardClick(event: MouseEvent) {
  if (store.selectionMode) {
    store.toggleSelection(props.wallpaper.path);
    return;
  }

  // Use event.detail to distinguish single-click (1) from double-click (2)
  // instead of a setTimeout debounce. This eliminates the 180ms artificial delay.
  if (event.detail === 2) {
    void store.setAsWallpaper(props.wallpaper.path);
    return;
  }

  store.setActiveWallpaper(props.wallpaper.path);
  openInspector(event.currentTarget as HTMLElement);
}

function onCardKeyboardActivate(event: KeyboardEvent) {
  // Only activate the card when the key event originated on the card itself.
  // Nested buttons (select, retry, rating, actions) handle their own keys; a
  // bubbling Enter/Space from them must not also activate the card.
  if (event.target !== event.currentTarget) {
    return;
  }
  if (store.selectionMode) {
    store.toggleSelection(props.wallpaper.path);
    return;
  }
  store.setActiveWallpaper(props.wallpaper.path);
  openInspector(event.currentTarget as HTMLElement);
}

function onCardNavigationKey(event: KeyboardEvent) {
  if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
    emit("moveFocus", event, props.wallpaper.id);
  }
}

function clearDeleteConfirm() {
  window.clearTimeout(deleteConfirmTimeout);
  deleteConfirmTimeout = undefined;
  showConfirmDelete.value = false;
}

async function handleDelete() {
  if (!showConfirmDelete.value) {
    showConfirmDelete.value = true;
    window.clearTimeout(deleteConfirmTimeout);
    deleteConfirmTimeout = window.setTimeout(() => {
      showConfirmDelete.value = false;
      deleteConfirmTimeout = undefined;
    }, 3000);
    return;
  }
  await store.deleteWallpaper(props.wallpaper.path);
  clearDeleteConfirm();
}

onBeforeUnmount(() => {
  window.clearTimeout(deleteConfirmTimeout);
});
</script>

<template>
  <article
    class="wallpaper-card"
    :class="[
      `wallpaper-card--${viewMode || 'grid'}`,
      { selected, active, liked: wallpaper.rating === 1, disabled: wallpaper.blacklisted },
    ]"
    role="gridcell"
    :aria-label="wallpaperAccessibleLabel(wallpaper)"
    :aria-current="active ? 'true' : undefined"
    aria-keyshortcuts="Enter Space"
    :tabindex="cardTabIndex ?? 0"
    :data-wallpaper-id="wallpaper.id"
    :aria-selected="selected"
    @mouseenter="isHovered = true"
    @mouseleave="isHovered = false; clearDeleteConfirm()"
    @click="onCardClick"
    @focusin="emit('focusCard', wallpaper.id)"
    @keydown="onCardNavigationKey"
    @keydown.enter="onCardKeyboardActivate"
    @keydown.space.prevent="onCardKeyboardActivate"
  >
    <div class="wallpaper-card__media">
      <button
        v-if="store.selectionMode || isHovered"
        class="select-dot"
        :class="{ active: selected }"
        type="button"
        :aria-pressed="selected"
        aria-label="Select wallpaper"
        @click.stop="store.toggleSelection(wallpaper.path)"
      >
        <AppIcon name="check" />
      </button>

      <div v-if="!thumbnailUrl" class="tile-loading">
        <button
          v-if="thumbnailError"
          class="thumbnail-retry"
          type="button"
          :title="thumbnailError"
          aria-label="Retry thumbnail"
          @click.stop="store.retryThumbnail(wallpaper.path)"
        >
          <AppIcon name="image" />
          <span>Retry</span>
        </button>
        <div v-else class="spinner small"></div>
      </div>

      <img
        v-else
        :src="thumbnailUrl"
        :alt="wallpaperAccessibleLabel(wallpaper)"
        @error="store.handleThumbnailLoadError(wallpaper.path, thumbnailUrl)"
      />

      <div class="card-state-badges">
        <span v-if="wallpaper.blacklisted">Hidden</span>
        <span v-if="wallpaper.rating === -1">Disliked</span>
      </div>

      <Transition name="fade">
        <div v-show="isHovered && !store.selectionMode" class="tile-overlay">
          <div class="tile-title">
            <span>{{ wallpaper.play_count }} plays</span>
          </div>
          <div class="tile-actions">
            <IconButton
              icon="heart"
              :active="wallpaper.rating === 1"
              :label="wallpaper.rating === 1 ? 'Unlike' : 'Like'"
              variant="quiet"
              @click.stop="wallpaper.rating === 1 ? store.resetRating(wallpaper.path) : store.like(wallpaper.path)"
            />
            <IconButton
              icon="dislike"
              :active="wallpaper.rating === -1"
              :label="wallpaper.rating === -1 ? 'Reset dislike' : 'Dislike'"
              variant="quiet"
              @click.stop="wallpaper.rating === -1 ? store.resetRating(wallpaper.path) : store.dislike(wallpaper.path)"
            />
            <IconButton icon="wallpaper" label="Set as wallpaper" variant="quiet" @click.stop="store.setAsWallpaper(wallpaper.path)" />
            <IconButton
              :icon="wallpaper.blacklisted ? 'library' : 'hidden'"
              :label="wallpaper.blacklisted ? 'Restore' : 'Hide'"
              variant="quiet"
              @click.stop="store.setBlacklisted(wallpaper.path, !wallpaper.blacklisted)"
            />
            <IconButton
              :icon="showConfirmDelete ? 'check' : 'trash'"
              :label="showConfirmDelete ? 'Confirm delete' : 'Delete'"
              variant="quiet"
              @click.stop="handleDelete"
            />
          </div>
        </div>
      </Transition>
    </div>

    <div v-if="visibleTags.length > 0" class="tile-tags">
      <span
        v-for="tag in visibleTags"
        :key="tag.id"
        :style="{ '--tag-color': tag.color }"
      >
        {{ tag.name }}
      </span>
      <span v-if="extraTagCount > 0">+{{ extraTagCount }}</span>
    </div>

    <div v-if="isListView" class="wallpaper-card__details">
      <div class="wallpaper-card__meta">
        <span>{{ wallpaper.play_count }} plays</span>
        <span v-if="wallpaper.rating === 1">Liked</span>
        <span v-else-if="wallpaper.rating === -1">Disliked</span>
        <span v-if="wallpaper.blacklisted">Hidden</span>
      </div>
    </div>
  </article>
</template>
