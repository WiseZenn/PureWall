<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import AppIcon from "./AppIcon.vue";

const props = withDefaults(
  defineProps<{
    colors: string[];
    compact?: boolean;
  }>(),
  {
    compact: false,
  },
);

const activeColor = ref("");
const copiedValue = ref("");
const popoverPosition = ref({ left: 0, top: 0 });
let hideTimer: number | undefined;

const colorValues = computed(() => {
  const hex = normalizeHex(activeColor.value);
  const rgb = hexToRgb(hex);
  return {
    hex,
    rgb: rgb ? `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})` : "",
    hsl: rgb ? rgbToHsl(rgb.r, rgb.g, rgb.b) : "",
  };
});

function normalizeHex(color: string) {
  const value = color.trim().replace(/^#/, "");
  if (value.length === 3) {
    return `#${value
      .split("")
      .map((part) => part + part)
      .join("")
      .toUpperCase()}`;
  }
  return `#${value.slice(0, 6).toUpperCase()}`;
}

function hexToRgb(color: string) {
  const value = color.replace(/^#/, "");
  if (!/^[0-9a-f]{6}$/i.test(value)) return null;
  return {
    r: Number.parseInt(value.slice(0, 2), 16),
    g: Number.parseInt(value.slice(2, 4), 16),
    b: Number.parseInt(value.slice(4, 6), 16),
  };
}

function rgbToHsl(r: number, g: number, b: number) {
  const red = r / 255;
  const green = g / 255;
  const blue = b / 255;
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const delta = max - min;
  let hue = 0;

  if (delta !== 0) {
    if (max === red) hue = ((green - blue) / delta) % 6;
    else if (max === green) hue = (blue - red) / delta + 2;
    else hue = (red - green) / delta + 4;
  }

  hue = Math.round(hue * 60);
  if (hue < 0) hue += 360;
  const lightness = (max + min) / 2;
  const saturation = delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1));
  return `hsl(${hue}, ${Math.round(saturation * 100)}%, ${Math.round(lightness * 100)}%)`;
}

function showPopover(color: string, event: MouseEvent | FocusEvent) {
  cancelHide();
  const button = event.currentTarget as HTMLElement;
  const rect = button.getBoundingClientRect();
  const width = 286;
  const left = Math.min(Math.max(12, rect.left + rect.width / 2 - width / 2), window.innerWidth - width - 12);
  const top = rect.bottom + 10;
  activeColor.value = color;
  popoverPosition.value = { left, top };
}

function cancelHide() {
  window.clearTimeout(hideTimer);
}

function scheduleHide() {
  cancelHide();
  hideTimer = window.setTimeout(() => {
    activeColor.value = "";
  }, 220);
}

async function copyValue(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    copiedValue.value = value;
    window.setTimeout(() => {
      if (copiedValue.value === value) copiedValue.value = "";
    }, 1400);
  } catch (e) {
    console.error("Failed to copy color:", e);
  }
}

async function selectColor(color: string, event: MouseEvent) {
  showPopover(color, event);
  await copyValue(normalizeHex(color));
}

onBeforeUnmount(() => window.clearTimeout(hideTimer));
</script>

<template>
  <div class="color-swatch-list" :class="{ compact: props.compact }" aria-label="Wallpaper colors">
    <button
      v-for="color in colors"
      :key="color"
      class="color-swatch"
      type="button"
      :style="{ background: color }"
      :aria-label="`Show and copy ${color}`"
      :aria-expanded="activeColor === color"
      @mouseenter="showPopover(color, $event)"
      @mouseleave="scheduleHide"
      @focus="showPopover(color, $event)"
      @blur="scheduleHide"
      @click="selectColor(color, $event)"
    ></button>
  </div>

  <Teleport to="body">
    <div
      v-if="activeColor"
      class="color-popover"
      role="dialog"
      aria-label="Color values"
      :style="{ left: `${popoverPosition.left}px`, top: `${popoverPosition.top}px` }"
      @mouseenter="cancelHide"
      @mouseleave="scheduleHide"
    >
      <div class="color-popover__header">
        <span class="color-popover__sample" :style="{ background: activeColor }"></span>
        <strong>Color</strong>
        <span>Click a value to copy</span>
      </div>
      <button
        v-for="(value, label) in colorValues"
        :key="label"
        class="color-value-row"
        type="button"
        @click="copyValue(value)"
      >
        <span>{{ label.toUpperCase() }}</span>
        <strong>{{ value }}</strong>
        <AppIcon :name="copiedValue === value ? 'check' : 'copy'" />
      </button>
    </div>
  </Teleport>
</template>
