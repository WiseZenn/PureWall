<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import AppIcon from "./AppIcon.vue";

interface DropdownOption {
  label: string;
  value: string | number;
  group?: string;
}

interface DropdownGroup {
  label?: string;
  entries: Array<{
    option: DropdownOption;
    index: number;
  }>;
}

const props = defineProps<{
  icon: string;
  label: string;
  modelValue: string | number;
  options: DropdownOption[];
  disabled?: boolean;
}>();

const emit = defineEmits<{
  change: [value: string | number];
}>();

const root = ref<HTMLElement | null>(null);
const menuId = `compact-dropdown-menu-${Math.random().toString(36).slice(2)}`;
const isOpen = ref(false);
const activeIndex = ref(0);
const selected = computed(() => props.options.find((option) => option.value === props.modelValue) ?? props.options[0]);
const activeOptionId = computed(() => `${menuId}-option-${activeIndex.value}`);
const groupedOptions = computed<DropdownGroup[]>(() => {
  const groups: DropdownGroup[] = [];
  props.options.forEach((option, index) => {
    const current = groups[groups.length - 1];
    if (!current || current.label !== option.group) {
      groups.push({
        label: option.group,
        entries: [{ option, index }],
      });
    } else {
      current.entries.push({ option, index });
    }
  });
  return groups;
});

function closeMenu() {
  isOpen.value = false;
}

function openMenu() {
  if (props.disabled) return;
  activeIndex.value = Math.max(0, props.options.findIndex((option) => option.value === props.modelValue));
  isOpen.value = true;
}

function toggleMenu() {
  if (isOpen.value) closeMenu();
  else openMenu();
}

function selectOption(option: DropdownOption) {
  emit("change", option.value);
  closeMenu();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    if (!isOpen.value) openMenu();
    const delta = event.key === "ArrowDown" ? 1 : -1;
    activeIndex.value = (activeIndex.value + delta + props.options.length) % props.options.length;
  } else if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    if (isOpen.value) selectOption(props.options[activeIndex.value]);
    else openMenu();
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
  <div ref="root" class="compact-dropdown" :class="{ open: isOpen }">
    <button
      class="compact-dropdown__trigger"
      type="button"
      :aria-label="label"
      :aria-expanded="isOpen"
      :aria-controls="menuId"
      :aria-activedescendant="isOpen ? activeOptionId : undefined"
      :disabled="disabled"
      @click="toggleMenu"
      @keydown="onKeydown"
    >
      <AppIcon :name="icon" />
      <span>{{ selected?.label }}</span>
      <AppIcon name="chevron-down" />
    </button>
    <div v-if="isOpen" :id="menuId" class="compact-dropdown__menu" role="listbox" :aria-label="label">
      <template v-for="(group, groupIndex) in groupedOptions" :key="`${group.label ?? 'options'}-${groupIndex}`">
        <div v-if="group.label" class="compact-dropdown__group" role="presentation">
          {{ group.label }}
        </div>
        <button
          v-for="{ option, index } in group.entries"
          :key="option.value"
          :id="`${menuId}-option-${index}`"
          class="compact-dropdown__option"
          :class="{ active: option.value === modelValue || activeIndex === index }"
          type="button"
          role="option"
          tabindex="-1"
          :aria-selected="option.value === modelValue"
          @pointerdown.prevent="selectOption(option)"
          @pointerenter="activeIndex = index"
        >
          <span>{{ option.label }}</span>
          <AppIcon v-if="option.value === modelValue" name="check" />
        </button>
      </template>
    </div>
  </div>
</template>
