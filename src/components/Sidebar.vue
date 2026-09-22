<script setup lang="ts">
import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useWallpaperStore } from "../stores/wallpapers";
import type { FilterKey, WorkspaceSection } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";
import SidebarItem from "./SidebarItem.vue";
import TagPill from "./TagPill.vue";
import {
  createCollectionFromDraft,
  sidebarFilters,
} from "./sidebarModel";
import { COMMAND_LINE_TITLE } from "./commandLineModel";
import { useInspector } from "../composables/useInspector";

const store = useWallpaperStore();
const { openInspector } = useInspector();
const newTagName = ref("");
const newCollectionName = ref("");
const tagInput = ref<HTMLInputElement | null>(null);
const collectionInput = ref<HTMLInputElement | null>(null);
const nextTagColor = ref(0);
const nextCollectionColor = ref(0);

const systemLinks = [
  { key: "displays", label: "Displays", icon: "display" },
  { key: "settings", label: "Settings", icon: "settings" },
  { key: "insights", label: "Insights", icon: "chart" },
  { key: "shortcuts", label: COMMAND_LINE_TITLE, icon: "shortcuts" },
  { key: "advanced", label: "Advanced", icon: "advanced" },
] satisfies Array<{
  key: WorkspaceSection;
  label: string;
  icon: string;
}>;

const tagPalette = ["#4cc9f0", "#7bd88f", "#f7c948", "#ff6b8a", "#b892ff", "#73d2de"];
const collectionPalette = ["#5b8def", "#ff7a59", "#37c871", "#d86cff", "#f5b84b", "#45c4b0"];

async function openSystemSection(section: WorkspaceSection, event: MouseEvent) {
  const opener = event.currentTarget as HTMLElement;
  store.setWorkspaceSection(section);
  openInspector(opener);
  await nextTick();
  document.querySelector<HTMLElement>(".system-inspector-shell")?.focus();
}

function isActive(filter: FilterKey | "tags") {
  if (filter === "tags") return store.currentFilter.startsWith("tag:");
  return store.currentFilter === filter;
}

function formatCount(value: number) {
  return value.toLocaleString();
}

async function selectFilter(filter: FilterKey | "tags") {
  if (filter === "tags") {
    const firstTag = store.tags[0];
    if (firstTag) await store.setFilter(`tag:${firstTag.id}`);
    return;
  }
  await store.setFilter(filter);
}

async function focusTagInput() {
  await nextTick();
  tagInput.value?.focus();
}

async function focusCollectionInput() {
  await nextTick();
  collectionInput.value?.focus();
}

async function addTag() {
  const name = newTagName.value.trim();
  if (!name) return;

  const color = tagPalette[nextTagColor.value % tagPalette.length];
  const tag = await store.createTag(name, color);
  if (tag) {
    newTagName.value = "";
    nextTagColor.value += 1;
  }
}

async function addCollection() {
  const color = collectionPalette[
    nextCollectionColor.value % collectionPalette.length
  ];
  const created = await createCollectionFromDraft(
    store,
    newCollectionName.value,
    color,
  );
  if (created) {
    newCollectionName.value = "";
    nextCollectionColor.value += 1;
  }
}

async function toggleWidget() {
  try {
    await invoke("toggle_widget");
  } catch (error) {
    console.error("Failed to toggle floating widget:", error);
  }
}
</script>

<template>
  <aside class="side-rail">
    <nav class="sidebar-nav" aria-label="Library navigation">
      <SidebarItem
        v-for="filter in sidebarFilters"
        :key="filter.key"
        :icon="filter.icon"
        :label="filter.label"
        :count="formatCount(filter.count(store.stats, store.tags.length))"
        :active="store.workspaceSection === 'library' && isActive(filter.key)"
        active-kind="pressed"
        @click="selectFilter(filter.key)"
      />
    </nav>

    <section class="sidebar-tags" aria-label="Tags">
      <div class="sidebar-section-heading">
        <span>Tags</span>
        <button type="button" aria-label="New tag" title="New tag" @click="focusTagInput">
          <AppIcon name="plus" />
        </button>
      </div>
      <div class="tag-pill-list">
        <button
          v-for="tag in store.tags"
          :key="tag.id"
          class="tag-pill-button"
          type="button"
          :aria-pressed="store.currentFilter === `tag:${tag.id}`"
          @click="store.setFilter(`tag:${tag.id}`)"
        >
          <TagPill
            :label="tag.name"
            :color="tag.color"
            :active="store.currentFilter === `tag:${tag.id}`"
          />
        </button>
      </div>

      <div class="tag-create">
        <input
          ref="tagInput"
          v-model="newTagName"
          type="text"
          maxlength="24"
          placeholder="New tag"
          aria-label="New tag name"
          @keydown.enter="addTag"
        />
        <button type="button" @click="addTag" aria-label="Create tag" title="Create tag">
          <AppIcon name="plus" />
        </button>
      </div>
    </section>

    <section class="sidebar-tags sidebar-collections" aria-label="Collections">
      <div class="sidebar-section-heading">
        <span>Collections</span>
        <button type="button" aria-label="New collection" title="New collection" @click="focusCollectionInput">
          <AppIcon name="plus" />
        </button>
      </div>
      <div class="tag-pill-list">
        <button
          v-for="collection in store.collections"
          :key="collection.id"
          class="tag-pill-button"
          type="button"
          :aria-pressed="store.currentFilter === `collection:${collection.id}`"
          :title="`${collection.name} - ${collection.wallpaper_count} wallpapers`"
          @click="store.setFilter(`collection:${collection.id}`)"
        >
          <TagPill
            :label="collection.name"
            :color="collection.color"
            :active="store.currentFilter === `collection:${collection.id}`"
          />
        </button>
      </div>

      <div class="tag-create">
        <input
          ref="collectionInput"
          v-model="newCollectionName"
          type="text"
          maxlength="24"
          placeholder="New collection"
          aria-label="New collection name"
          @keydown.enter="addCollection"
        />
        <button type="button" @click="addCollection" aria-label="Create collection" title="Create collection">
          <AppIcon name="plus" />
        </button>
      </div>
    </section>

    <section class="sidebar-system" aria-label="System">
      <h2 class="sidebar-heading">System</h2>
      <SidebarItem
        icon="widget"
        label="Floating widget"
        :active="false"
        active-kind="none"
        quiet
        @click="toggleWidget"
      />
      <SidebarItem
        v-for="item in systemLinks"
        :key="item.label"
        :icon="item.icon"
        :label="item.label"
        :active="store.workspaceSection === item.key"
        active-kind="current"
        quiet
        @click="openSystemSection(item.key, $event)"
      />
    </section>
  </aside>
</template>
