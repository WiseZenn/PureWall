<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

withDefaults(
  defineProps<{
    icon: string;
    label: string;
    active?: boolean;
    disabled?: boolean;
    variant?: "default" | "quiet" | "primary";
  }>(),
  {
    active: false,
    disabled: false,
    variant: "default",
  },
);

defineEmits<{
  click: [event: MouseEvent];
}>();
</script>

<template>
  <button
    class="icon-button"
    :class="[`icon-button--${variant}`, { active }]"
    :disabled="disabled"
    :aria-label="label"
    :aria-pressed="active ? 'true' : undefined"
    :title="label"
  >
    <AppIcon :name="icon" />
  </button>
</template>
