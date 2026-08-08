<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTheme } from "../composables/useTheme";
import AppIcon from "./AppIcon.vue";

const autostartEnabled = ref(false);
const autostartKnown = ref(false);
const { theme, cycleTheme } = useTheme();

const themeLabel = computed(() => {
  if (theme.value === "system") return "System";
  return theme.value === "dark" ? "Dark" : "Light";
});

const themeIcon = computed(() => {
  if (theme.value === "system") return "desktop";
  return theme.value === "dark" ? "theme-dark" : "theme-light";
});

onMounted(async () => {
  try {
    autostartEnabled.value = await invoke<boolean>("is_autostart_enabled");
    autostartKnown.value = true;
  } catch {
    autostartKnown.value = false;
  }
});
</script>

<template>
  <footer class="statusbar-shell">
    <div class="status-left">
      <span class="status-dot"></span>
      <span>Ready</span>
      <button
        class="status-theme-toggle"
        type="button"
        :aria-label="`Theme: ${themeLabel}. Click to switch theme.`"
        :title="`Theme: ${themeLabel}. Click to switch theme.`"
        @click="cycleTheme"
      >
        <AppIcon :name="themeIcon" />
        <span>{{ themeLabel }}</span>
      </button>
    </div>
    <div class="status-right">
      <span>Auto-start</span>
      <strong>{{ autostartKnown ? (autostartEnabled ? "On" : "Off") : "Unknown" }}</strong>
    </div>
  </footer>
</template>
