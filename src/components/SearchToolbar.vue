<script setup lang="ts">
import { useWallpaperStore } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";
import IconButton from "./IconButton.vue";

const store = useWallpaperStore();

function clearSearch() {
  if (store.searchQuery) {
    store.searchQuery = "";
  }
}
</script>

<template>
  <div class="search-toolbar">
    <label class="search-field">
      <span class="sr-only">Search wallpapers</span>
      <AppIcon name="search" />
      <input v-model="store.searchQuery" type="search" placeholder="Search wallpapers..." />
    </label>
    <IconButton
      v-if="store.searchQuery"
      :icon="store.searchQuery ? 'x' : 'filter'"
      label="Clear search"
      active
      @click="clearSearch"
    />
  </div>
</template>
