<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

withDefaults(
  defineProps<{
    icon: string;
    label: string;
    count?: string | number;
    active?: boolean;
    quiet?: boolean;
    activeKind?: "current" | "pressed" | "none";
  }>(),
  {
    active: false,
    quiet: false,
    activeKind: "none",
  },
);
</script>

<template>
  <button
    class="sidebar-item"
    :class="{ active, quiet }"
    type="button"
    :aria-current="active && activeKind === 'current' ? 'page' : undefined"
    :aria-pressed="activeKind === 'pressed' ? active : undefined"
  >
    <span class="sidebar-item__accent" aria-hidden="true"></span>
    <AppIcon :name="icon" />
    <span class="sidebar-item__label">{{ label }}</span>
    <span v-if="count !== undefined" class="sidebar-item__count">{{ count }}</span>
  </button>
</template>
