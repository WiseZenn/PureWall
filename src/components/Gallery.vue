<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { useWallpaperStore } from "../stores/wallpapers";
import type { SortMode, WallpaperViewMode } from "../stores/wallpapers";
import {
  buildBatchRelationOptions,
  parseBatchRelationValue,
} from "./batchRelationMenuModel";
import AppIcon from "./AppIcon.vue";
import CompactDropdown from "./CompactDropdown.vue";
import CurrentWallpaperPanel from "./CurrentWallpaperPanel.vue";
import WallpaperGrid from "./WallpaperGrid.vue";

const store = useWallpaperStore();
const batchTagAction = ref("");
const batchCollectionAction = ref("");
const newCollectionName = ref("");
const collectionColorIndex = ref(0);
const confirmDelete = ref(false);
let confirmDeleteTimeout: number | undefined;

const sortOptions: Array<{ key: SortMode; label: string }> = [
  { key: "created", label: "Date added" },
  { key: "liked", label: "Liked" },
  { key: "plays", label: "Plays" },
  { key: "recent", label: "Recent" },
];

function setSort(event: Event) {
  store.setSort((event.target as HTMLSelectElement).value as SortMode);
}

const viewModes: Array<{ key: WallpaperViewMode; label: string; icon: string }> = [
  { key: "grid", label: "Grid view", icon: "grid" },
  { key: "list", label: "List view", icon: "list" },
  { key: "compact", label: "Compact view", icon: "compact" },
];

const collectionPalette = ["#4cc9f0", "#7bd88f", "#f7c948", "#ff6b8a", "#b892ff", "#73d2de"];

const batchTagOptions = computed(() =>
  buildBatchRelationOptions(store.tags, "Tags"),
);
const batchCollectionOptions = computed(() =>
  buildBatchRelationOptions(store.collections, "Collections"),
);

function toggleSelectionMode() {
  if (store.isBatchMutating) return;
  if (store.selectionMode && store.selectedCount > 0) {
    store.clearSelection();
  } else {
    store.selectionMode = !store.selectionMode;
  }
}

async function applyBatchTag(value: string | number) {
  batchTagAction.value = String(value);
  const action = parseBatchRelationValue(value);
  if (!action) return;

  const succeeded = action.operation === "assign"
    ? await store.batchAssignTag(action.id)
    : await store.batchUnassignTag(action.id);
  if (succeeded) batchTagAction.value = "";
}

async function applyBatchCollection(value: string | number) {
  batchCollectionAction.value = String(value);
  const action = parseBatchRelationValue(value);
  if (!action) return;

  const succeeded = action.operation === "assign"
    ? await store.batchAssignCollection(action.id)
    : await store.batchUnassignCollection(action.id);
  if (succeeded) batchCollectionAction.value = "";
}

async function createCollectionFromSelection() {
  const name = newCollectionName.value.trim();
  if (!name || store.selectedCount === 0 || store.isBatchMutating) return;

  const color = collectionPalette[collectionColorIndex.value % collectionPalette.length];
  const collection = await store.createCollection(name, color);
  if (!collection) return;

  collectionColorIndex.value += 1;
  newCollectionName.value = "";
  await store.batchAssignCollection(collection.id);
}

async function deleteSelected() {
  if (store.isBatchMutating) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    window.clearTimeout(confirmDeleteTimeout);
    confirmDeleteTimeout = window.setTimeout(() => {
      confirmDelete.value = false;
      confirmDeleteTimeout = undefined;
    }, 3000);
    return;
  }

  await store.batchDelete();
  window.clearTimeout(confirmDeleteTimeout);
  confirmDeleteTimeout = undefined;
  confirmDelete.value = false;
}

onBeforeUnmount(() => {
  window.clearTimeout(confirmDeleteTimeout);
});
</script>

<template>
  <div class="gallery-workspace">
    <CurrentWallpaperPanel />

    <div class="gallery-toolbar">
      <div class="collection-heading">
        <h2 id="gallery-heading">Your collection</h2>
        <span>{{ store.wallpaperTotal.toLocaleString() }} wallpapers</span>
      </div>

      <label class="toolbar-select">
        <AppIcon name="sort" />
        <span>Sort</span>
        <select :value="store.sortMode" aria-label="Sort wallpapers" @change="setSort">
          <option v-for="option in sortOptions" :key="option.key" :value="option.key">
            {{ option.label }}
          </option>
        </select>
      </label>

      <button
        class="toolbar-tab standalone"
        :class="{ active: store.selectionMode }"
        type="button"
        :disabled="store.isBatchMutating"
        @click="toggleSelectionMode"
      >
        <AppIcon name="select" />
        {{ store.selectionMode ? "Done" : "Select" }}
      </button>

      <div class="view-buttons" aria-label="View mode">
        <button
          v-for="mode in viewModes"
          :key="mode.key"
          :class="{ active: store.wallpaperViewMode === mode.key }"
          type="button"
          :aria-label="mode.label"
          :aria-pressed="store.wallpaperViewMode === mode.key"
          :title="mode.label"
          @click="store.setWallpaperViewMode(mode.key)"
        >
          <AppIcon :name="mode.icon" />
        </button>
      </div>

      <div v-if="store.selectedCount > 0" class="selection-bar">
        <span>{{ store.selectedCount }} selected</span>
        <button type="button" :disabled="store.isBatchMutating" @click="store.batchSetRating(1)" title="Like selected" aria-label="Like selected">
          <AppIcon name="heart" />
        </button>
        <button type="button" :disabled="store.isBatchMutating" @click="store.batchSetRating(-1)" title="Dislike selected" aria-label="Dislike selected">
          <AppIcon name="dislike" />
        </button>
        <button type="button" :disabled="store.isBatchMutating" @click="store.batchSetRating(0)" title="Clear selected ratings" aria-label="Clear selected ratings">
          <AppIcon name="x" />
        </button>
        <CompactDropdown
          icon="tag"
          label="Manage tags for selected wallpapers"
          :model-value="batchTagAction"
          :options="batchTagOptions"
          :disabled="store.isBatchMutating || store.tags.length === 0"
          @change="applyBatchTag"
        />
        <CompactDropdown
          icon="library"
          label="Manage collections for selected wallpapers"
          :model-value="batchCollectionAction"
          :options="batchCollectionOptions"
          :disabled="store.isBatchMutating || store.collections.length === 0"
          @change="applyBatchCollection"
        />
        <div class="selection-create-collection">
          <input
            v-model="newCollectionName"
            type="text"
            maxlength="24"
            :disabled="store.isBatchMutating"
            placeholder="New collection"
            aria-label="New collection name"
            @keydown.enter="createCollectionFromSelection"
          />
          <button
            type="button"
            :disabled="store.isBatchMutating || !newCollectionName.trim()"
            title="Create collection from selected"
            aria-label="Create collection from selected"
            @click="createCollectionFromSelection"
          >
            <AppIcon name="plus" />
          </button>
        </div>
        <button v-if="store.currentFilter === 'blacklisted'" type="button" :disabled="store.isBatchMutating" @click="store.batchBlacklist(false)">
          Restore
        </button>
        <button v-else type="button" :disabled="store.isBatchMutating" @click="store.batchBlacklist(true)">Hide</button>
        <button type="button" :disabled="store.isBatchMutating" :class="{ danger: confirmDelete }" @click="deleteSelected">
          {{ confirmDelete ? "Confirm" : "Delete" }}
        </button>
      </div>
    </div>

    <WallpaperGrid />
  </div>
</template>
