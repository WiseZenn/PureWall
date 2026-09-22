<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useWallpaperStore, type TagEntry } from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";


const store = useWallpaperStore();
const root = ref<HTMLElement | null>(null);
const listboxId = `tag-combobox-listbox-${Math.random().toString(36).slice(2)}`;
const query = ref("");
const isOpen = ref(false);
const activeIndex = ref(0);
const isSubmitting = ref(false);

const normalizedQuery = computed(() => query.value.trim().toLocaleLowerCase());
const unassignedTags = computed(() => {
  const assigned = new Set(store.activeWallpaper?.tags.map((tag) => tag.id) ?? []);
  return store.tags.filter((tag) => !assigned.has(tag.id));
});
const matches = computed(() => {
  const value = normalizedQuery.value;
  return unassignedTags.value
    .filter((tag) => !value || tag.name.toLocaleLowerCase().includes(value))
    .sort((a, b) => {
      const aStarts = a.name.toLocaleLowerCase().startsWith(value);
      const bStarts = b.name.toLocaleLowerCase().startsWith(value);
      if (aStarts !== bStarts) return aStarts ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
});
const hasExactTag = computed(() =>
  store.tags.some((tag) => tag.name.toLocaleLowerCase() === normalizedQuery.value),
);
const canCreate = computed(() => normalizedQuery.value.length > 0 && !hasExactTag.value);
const optionCount = computed(() => matches.value.length + (canCreate.value ? 1 : 0));
const activeOptionId = computed(() => `${listboxId}-option-${activeIndex.value}`);

watch(query, () => {
  activeIndex.value = 0;
});

function closeMenu() {
  isOpen.value = false;
  activeIndex.value = 0;
}

function reset() {
  query.value = "";
  closeMenu();
}

async function assign(tag: TagEntry) {
  if (!store.activeWallpaper || isSubmitting.value) return;
  isSubmitting.value = true;
  await store.assignTag(store.activeWallpaper.path, tag.id);
  isSubmitting.value = false;
  reset();
}

async function createAndAssign() {
  const name = query.value.trim();
  if (!store.activeWallpaper || !name || !canCreate.value || isSubmitting.value) return;
  isSubmitting.value = true;
  const tag = await store.createTag(name, "#087F75");
  if (tag) await store.assignTag(store.activeWallpaper.path, tag.id);
  isSubmitting.value = false;
  reset();
}

async function selectActiveOption() {
  const tag = matches.value[activeIndex.value];
  if (tag) await assign(tag);
  else if (canCreate.value && activeIndex.value === matches.value.length) await createAndAssign();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    isOpen.value = true;
    const count = Math.max(1, optionCount.value);
    const delta = event.key === "ArrowDown" ? 1 : -1;
    activeIndex.value = (activeIndex.value + delta + count) % count;
  } else if (event.key === "Enter") {
    event.preventDefault();
    void selectActiveOption();
  } else if (event.key === "Escape") {
    closeMenu();
  } else if (event.key === "Tab") {
    closeMenu();
  }
}

function onDocumentPointerDown(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) closeMenu();
}

onMounted(() => document.addEventListener("pointerdown", onDocumentPointerDown));
onBeforeUnmount(() => document.removeEventListener("pointerdown", onDocumentPointerDown));
</script>

<template>
  <div ref="root" class="tag-combobox" :class="{ open: isOpen }">
    <div class="tag-combobox__field">
      <AppIcon name="tag-search" />
      <input
        v-model="query"
        type="text"
        maxlength="24"
        placeholder="Add or create tag..."
        aria-label="Add or create tag"
        role="combobox"
        aria-autocomplete="list"
        :aria-expanded="isOpen"
        :aria-controls="listboxId"
        :aria-activedescendant="isOpen && optionCount > 0 ? activeOptionId : undefined"
        :disabled="isSubmitting"
        @focus="isOpen = true"
        @input="isOpen = true"
        @keydown="onKeydown"
      />
      <AppIcon name="chevron-down" />
    </div>
    <div v-if="isOpen && optionCount > 0" :id="listboxId" class="tag-combobox__menu" role="listbox">
      <button
        v-for="(tag, index) in matches"
        :key="tag.id"
        :id="`${listboxId}-option-${index}`"
        class="tag-combobox__option"
        :class="{ active: activeIndex === index }"
        type="button"
        role="option"
        tabindex="-1"
        :aria-selected="activeIndex === index"
        @pointerdown.prevent="assign(tag)"
        @pointerenter="activeIndex = index"
      >
        <span class="tag-combobox__dot" :style="{ backgroundColor: tag.color }"></span>
        <span>{{ tag.name }}</span>
      </button>
      <button
        v-if="canCreate"
        :id="`${listboxId}-option-${matches.length}`"
        class="tag-combobox__option create"
        :class="{ active: activeIndex === matches.length }"
        type="button"
        role="option"
        tabindex="-1"
        :aria-selected="activeIndex === matches.length"
        @pointerdown.prevent="createAndAssign"
        @pointerenter="activeIndex = matches.length"
      >
        <AppIcon name="plus" />
        <span>Create "{{ query.trim() }}"</span>
      </button>
    </div>
    <div v-else-if="isOpen && normalizedQuery" class="tag-combobox__menu empty">No matching tags</div>
  </div>
</template>
