<script setup lang="ts">
defineProps<{
  label: string;
  note?: string;
}>();
</script>

<template>
  <label class="setting-row">
    <span class="setting-row__label">{{ label }}</span>
    <span v-if="note" class="setting-row__note">{{ note }}</span>
    <span class="setting-row__control">
      <slot></slot>
    </span>
  </label>
</template>
