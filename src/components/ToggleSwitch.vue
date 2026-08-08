<script setup lang="ts">
defineProps<{
  checked: boolean;
  label: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  change: [checked: boolean];
}>();
</script>

<template>
  <button
    class="toggle-switch"
    :class="{ checked }"
    :disabled="disabled"
    type="button"
    role="switch"
    :aria-checked="checked"
    :aria-label="label"
    @click="emit('change', !checked)"
  >
    <span></span>
  </button>
</template>
