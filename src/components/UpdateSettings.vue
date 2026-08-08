<script setup lang="ts">
import { computed } from "vue";
import { useAppUpdatesStore } from "../stores/appUpdates";

const updates = useAppUpdatesStore();
const state = computed(() => updates.state);
const isBusy = computed(
  () => state.value.status === "checking" || state.value.status === "downloading",
);
const availableUpdate = computed(() => {
  const value = state.value;
  return value.status === "available" || value.status === "downloading" || value.status === "readyToRestart" || value.status === "completed"
    ? value.update
    : null;
});
const progress = computed(() =>
  state.value.status === "downloading" ? state.value.progress : null,
);

function formatBytes(value: number | null | undefined) {
  if (value === null || value === undefined) return "Unknown size";
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB"];
  let size = value / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && size >= 1024; index += 1) {
    size /= 1024;
    unit = units[index];
  }
  return `${size >= 10 ? size.toFixed(1) : size.toFixed(2)} ${unit}`;
}

function formatDate(value: string | null) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleDateString();
}
</script>

<template>
  <section class="update-settings" aria-labelledby="update-settings-title">
    <div class="update-settings__header">
      <div>
        <h3 id="update-settings-title">Application updates</h3>
        <p class="update-settings__version">Current version {{ updates.currentVersion }}</p>
      </div>
      <button
        class="mini-action"
        type="button"
        :disabled="isBusy"
        @click="updates.checkForUpdates"
      >
        {{ state.status === "checking" ? "Checking…" : "Check for updates" }}
      </button>
    </div>

    <div v-if="state.status === 'current'" class="update-settings__message" role="status">
      You are up to date.
    </div>

    <div v-if="availableUpdate" class="update-settings__available">
      <div class="update-settings__release">
        <strong>PureWall {{ availableUpdate.version }}</strong>
        <span v-if="availableUpdate.date">Released {{ formatDate(availableUpdate.date) }}</span>
      </div>
      <p v-if="availableUpdate.body" class="update-settings__notes">{{ availableUpdate.body }}</p>
      <button
        v-if="state.status === 'available'"
        class="mini-action primary"
        type="button"
        :disabled="isBusy"
        @click="updates.installUpdate"
      >
        Download and install
      </button>
      <div v-else-if="state.status === 'downloading'" class="update-settings__progress" role="status">
        <div class="update-settings__progress-label">
          <span>Downloading update</span>
          <span>{{ formatBytes(progress?.downloaded) }}<template v-if="progress?.contentLength"> / {{ formatBytes(progress.contentLength) }}</template></span>
        </div>
        <progress
          :value="progress?.downloaded"
          :max="progress?.contentLength || undefined"
          aria-label="Update download progress"
        />
      </div>
      <div v-else-if="state.status === 'readyToRestart' || state.status === 'completed'" class="update-settings__message" role="status">
        Update installed. PureWall will restart when Windows finishes the installer.
      </div>
    </div>

    <div v-if="state.status === 'error'" class="update-settings__error" role="alert">
      <span>{{ state.message }}</span>
      <button class="mini-action" type="button" @click="updates.retry">Retry</button>
    </div>

    <p class="update-settings__warning">
      Windows will close PureWall while the installer runs. Save any work before installing.
    </p>
  </section>
</template>
