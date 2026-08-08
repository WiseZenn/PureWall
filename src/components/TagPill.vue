<script setup lang="ts">
withDefaults(
  defineProps<{
    label: string;
    active?: boolean;
    color?: string;
    removable?: boolean;
  }>(),
  {
    active: false,
    removable: false,
  },
);
</script>

<template>
  <span
    class="tag-pill"
    :class="{ active, removable }"
    :style="color ? { '--tag-color': color } : undefined"
  >
    {{ label }}
  </span>
</template>
