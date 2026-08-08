<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useElementSize, useVirtualList } from "@vueuse/core";
import { useWallpaperStore } from "../stores/wallpapers";
import type { WallpaperEntry } from "../stores/wallpapers";
import { createThumbnailRequestScheduler } from "../utils/thumbnailRequestScheduler";
import AppIcon from "./AppIcon.vue";
import WallpaperCard from "./WallpaperCard.vue";

const store = useWallpaperStore();
const GRID_MIN_CARD_WIDTH = 190;
const COMPACT_MIN_CARD_WIDTH = 132;
const GRID_GAP = 12;
const COMPACT_GAP = 8;
const LIST_ROW_HEIGHT = 108;
const OVERSCAN_ROWS = 5;
const focusedWallpaperId = ref<number | null>(null);
const galleryScroller = ref<HTMLElement | null>(null);
const { width: galleryWidth } = useElementSize(galleryScroller);
const thumbnailRequestScheduler = createThumbnailRequestScheduler(
  (paths) => store.loadThumbnailsForPaths(paths),
  80,
);

type WallpaperRow = WallpaperEntry[];

const galleryGap = computed(() =>
  store.wallpaperViewMode === "compact" || store.wallpaperViewMode === "list" ? COMPACT_GAP : GRID_GAP,
);

const columnCount = computed(() => {
  if (store.wallpaperViewMode === "list") return 1;

  const availableWidth = Math.max(galleryWidth.value || 0, 1);
  const minCardWidth = store.wallpaperViewMode === "compact" ? COMPACT_MIN_CARD_WIDTH : GRID_MIN_CARD_WIDTH;
  return Math.max(1, Math.floor((availableWidth + galleryGap.value) / (minCardWidth + galleryGap.value)));
});

const virtualRowContentHeight = computed(() => {
  if (store.wallpaperViewMode === "list") return LIST_ROW_HEIGHT;

  const availableWidth = Math.max(galleryWidth.value || 0, columnCount.value * GRID_MIN_CARD_WIDTH);
  const totalGap = galleryGap.value * Math.max(0, columnCount.value - 1);
  const cardWidth = Math.max(1, (availableWidth - totalGap) / columnCount.value);
  return Math.ceil((cardWidth * 9) / 16) + 2;
});

const virtualRowHeight = computed(() => virtualRowContentHeight.value + galleryGap.value);

const wallpaperRows = computed<WallpaperRow[]>(() => {
  const rows: WallpaperRow[] = [];
  const columns = columnCount.value;
  const wallpapers = store.visibleWallpapers;

  for (let index = 0; index < wallpapers.length; index += columns) {
    rows.push(wallpapers.slice(index, index + columns));
  }

  return rows;
});

const orderedWallpapers = computed(() => store.visibleWallpapers);
const ariaRowCount = computed(() =>
  Math.ceil(store.wallpaperTotal / columnCount.value),
);
const virtualGridStyle = computed(() => ({
  gridTemplateColumns: `repeat(${columnCount.value}, minmax(0, 1fr))`,
  gap: `${galleryGap.value}px`,
  minHeight: `${virtualRowContentHeight.value}px`,
  marginBottom: `${galleryGap.value}px`,
}));

const {
  list: virtualRows,
  scrollTo,
  containerProps,
  wrapperProps,
} = useVirtualList(wallpaperRows, {
  itemHeight: () => virtualRowHeight.value,
  overscan: OVERSCAN_ROWS,
});

const visibleVirtualWallpapers = computed(() => virtualRows.value.flatMap((row) => row.data));

watch(
  () => [
    store.currentFilter,
    store.sortMode,
    store.searchQuery,
    store.wallpaperViewMode,
  ],
  () => {
    scrollTo(0);
  },
);

watch(
  visibleVirtualWallpapers,
  (wallpapers) => {
    const visiblePaths = wallpapers.map((wp) => wp.path);
    // Always debounce the latest visible set. The store removes cache hits and
    // in-flight paths; keeping only newly seen paths here can lose requests when
    // a layout recalculation cancels the previous timer.
    thumbnailRequestScheduler.schedule(visiblePaths);

    if (orderedWallpapers.value.length === 0) {
      focusedWallpaperId.value = null;
      return;
    }

    const activeWallpaper = orderedWallpapers.value.find(
      (wallpaper) => wallpaper.path === store.activeWallpaperPath,
    );
    const hasFocusedWallpaper = orderedWallpapers.value.some(
      (wallpaper) => wallpaper.id === focusedWallpaperId.value,
    );
    if (!hasFocusedWallpaper) {
      focusedWallpaperId.value = activeWallpaper?.id ?? orderedWallpapers.value[0].id;
    }

    void nextTick(() => {
      maybeLoadMoreWallpapers();
    });
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  thumbnailRequestScheduler.cancel();
});

function maybeLoadMoreWallpapers() {
  if (!store.hasMoreWallpapers || store.isLoadingMoreWallpapers) return;
  const scroller = galleryScroller.value;
  if (!scroller) return;

  const distanceToBottom = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
  const preloadDistance = Math.max(virtualRowHeight.value * 8, 900);
  if (distanceToBottom <= preloadDistance) {
    void store.loadMoreWallpapers();
  }
}

function handleGalleryScroll() {
  containerProps.onScroll();
  maybeLoadMoreWallpapers();
}
function setGalleryScroller(el: unknown) {
  const element = el instanceof HTMLElement ? el : null;
  galleryScroller.value = element;
  containerProps.ref.value = element;
}

function cardTabIndex(wallpaper: WallpaperEntry) {
  return wallpaper.id === focusedWallpaperId.value ? 0 : -1;
}

function focusCard(id: number) {
  focusedWallpaperId.value = id;
}

function rowIndexForWallpaperId(id: number) {
  const wallpaperIndex = orderedWallpapers.value.findIndex((wallpaper) => wallpaper.id === id);
  return wallpaperIndex === -1 ? -1 : Math.floor(wallpaperIndex / columnCount.value);
}

function focusWallpaperById(id: number) {
  focusedWallpaperId.value = id;
  const rowIndex = rowIndexForWallpaperId(id);
  if (rowIndex >= 0) {
    scrollTo(rowIndex);
  }

  void nextTick(() => {
    document.querySelector<HTMLElement>(`.wallpaper-card[data-wallpaper-id="${id}"]`)?.focus();
  });
}

function moveCardFocus(event: KeyboardEvent, currentId: number) {
  const wallpapers = orderedWallpapers.value;
  if (wallpapers.length === 0) return;

  const currentIndex = wallpapers.findIndex((wallpaper) => wallpaper.id === currentId);
  if (currentIndex === -1) return;

  const columns = columnCount.value;
  let nextIndex = currentIndex;
  if (event.key === "Home") {
    nextIndex = 0;
  } else if (event.key === "End") {
    nextIndex = wallpapers.length - 1;
  } else if (event.key === "ArrowRight") {
    nextIndex = Math.min(currentIndex + 1, wallpapers.length - 1);
  } else if (event.key === "ArrowLeft") {
    nextIndex = Math.max(currentIndex - 1, 0);
  } else if (event.key === "ArrowDown") {
    nextIndex = Math.min(currentIndex + columns, wallpapers.length - 1);
  } else if (event.key === "ArrowUp") {
    nextIndex = Math.max(currentIndex - columns, 0);
  }

  const nextId = wallpapers[nextIndex]?.id;
  if (nextId !== undefined && nextId !== currentId) {
    event.preventDefault();
    focusWallpaperById(nextId);
  }
}
</script>

<template>
  <div
    class="gallery-scroll"
    role="grid"
    aria-label="Wallpaper library"
    :aria-rowcount="ariaRowCount"
    :aria-busy="store.isLoading || store.isLoadingMoreWallpapers"
    :ref="setGalleryScroller"
    :style="containerProps.style"
    @scroll.passive="handleGalleryScroll"
  >
    <div v-if="store.isLoading" class="loading-state">
      <div class="spinner"></div>
      <span>Loading</span>
    </div>

    <div v-else-if="store.visibleWallpapers.length === 0" class="empty-gallery">
      <AppIcon name="search" />
      <p>No wallpapers match this view</p>
      <button
        v-if="store.searchQuery || store.currentFilter !== 'all'"
        type="button"
        class="empty-gallery__reset"
        @click="store.searchQuery = ''; store.setFilter('all')"
      >
        Clear filters
      </button>
    </div>

    <template v-else>
      <div class="virtual-gallery-wrapper" v-bind="wrapperProps">
        <div
          v-for="row in virtualRows"
          :key="`${store.wallpaperViewMode}-${row.index}-${row.data[0]?.id ?? 'empty'}`"
          class="wallpaper-grid virtual-gallery-row"
          :class="`wallpaper-grid--${store.wallpaperViewMode}`"
          :style="virtualGridStyle"
          role="row"
          :aria-rowindex="row.index + 1"
        >
          <WallpaperCard
            v-for="wp in row.data"
            :key="wp.id"
            :wallpaper="wp"
            :card-tab-index="cardTabIndex(wp)"
            :view-mode="store.wallpaperViewMode"
            @focus-card="focusCard"
            @move-focus="moveCardFocus"
          />
        </div>
      </div>

      <div v-if="store.isLoadingMoreWallpapers" class="gallery-page-loading">
        <div class="spinner small"></div>
        <span>Loading more</span>
      </div>
    </template>
  </div>
</template>
