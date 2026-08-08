<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

defineProps<{
  icon: string;
  label: string;
  value: string;
  title?: string;
}>();
</script>

<template>
  <div class="metadata-row">
    <AppIcon :name="icon" />
    <span>{{ label }}</span>
    <strong :title="title || value">{{ value }}</strong>
  </div>
</template>
