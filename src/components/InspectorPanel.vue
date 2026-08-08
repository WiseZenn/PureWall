<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open as openPath } from "@tauri-apps/plugin-shell";
import { useWallpaperStore } from "../stores/wallpapers";
import type { DisplayMode } from "../stores/wallpapers";
import { displayModeOptions } from "../utils/displayModes";
import { useTheme, type ThemeMode } from "../composables/useTheme";
import { useWorkspaceMode } from "../composables/useWorkspaceMode";
import { useInspector } from "../composables/useInspector";
import AppIcon from "./AppIcon.vue";
import IconButton from "./IconButton.vue";
import LibraryBackupSettings from "./LibraryBackupSettings.vue";
import LibrarySourcesSettings from "./LibrarySourcesSettings.vue";
import MetadataRow from "./MetadataRow.vue";
import TagCombobox from "./TagCombobox.vue";
import TagPill from "./TagPill.vue";
import YearlyInsightsChart from "./YearlyInsightsChart.vue";
import UpdateSettings from "./UpdateSettings.vue";
import {
  COMMAND_LINE_SUBTITLE,
  COMMAND_LINE_TITLE,
  commandLineRows,
} from "./commandLineModel";

const store = useWallpaperStore();
const { theme, setTheme } = useTheme();
const { workspaceMode, setWorkspaceMode } = useWorkspaceMode();
const { closeInspector } = useInspector();
const copiedPath = ref(false);
const titleDraft = ref("");
const titleStatus = ref<"idle" | "saving" | "saved">("idle");
const systemPanelTitleId = computed(
  () => `system-panel-${store.workspaceSection}-title`,
);

const topCategory = computed(() => store.activeWallpaper?.tags[0]?.name || "Unassigned");
const displaySummary = computed(() => {
  const count = store.displays.length;
  if (count === 0) return "No display data";
  return `${count} ${count === 1 ? "display" : "displays"}`;
});

function folderName(path: string): string {
  const parts = path.split(/[\\/]/);
  parts.pop();
  return parts.join("\\") || path;
}

async function openContainingFolder(path: string) {
  await openPath(folderName(path));
}

async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    copiedPath.value = true;
    window.setTimeout(() => {
      copiedPath.value = false;
    }, 1400);
  } catch (e) {
    console.error("Failed to copy path:", e);
  }
}

function formatDate(value: string | null) {
  if (!value) return "Not played yet";
  return new Date(value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function formatBytes(value: number | undefined) {
  if (value === undefined) return "Unknown";
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

function setDisplayMode(mode: DisplayMode) {
  store.setDisplayMode(mode);
}

function selectTheme(mode: ThemeMode) {
  setTheme(mode);
}

async function saveDisplayTitle(force = false) {
  const wallpaper = store.activeWallpaper;
  if (!wallpaper || titleStatus.value === "saving") return;

  const nextTitle = titleDraft.value.trim();
  if (!force && nextTitle === wallpaper.display_title) return;

  titleStatus.value = "saving";
  const updated = await store.saveDisplayTitle(wallpaper.path, nextTitle);
  if (updated) {
    titleDraft.value = updated.display_title;
    titleStatus.value = "saved";
    window.setTimeout(() => {
      if (titleStatus.value === "saved") titleStatus.value = "idle";
    }, 1200);
  } else {
    titleStatus.value = "idle";
  }
}

async function closePanel() {
  store.setWorkspaceSection("library");
  await closeInspector();
}

function handlePanelKeydown(event: KeyboardEvent) {
  if (
    event.key !== "Escape" ||
    event.defaultPrevented ||
    store.workspaceSection === "library" ||
    document.querySelector('[role="dialog"]')
  ) {
    return;
  }

  event.preventDefault();
  void closePanel();
}

onMounted(() => window.addEventListener("keydown", handlePanelKeydown));

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handlePanelKeydown);
});

watch(
  () => store.activeWallpaper?.path,
  () => {
    titleDraft.value = store.activeWallpaper?.display_title ?? "";
    titleStatus.value = "idle";
  },
  { immediate: true },
);

watch(
  () => store.workspaceSection,
  (section) => {
    if (section === "advanced") {
      void store.loadCliLog();
    }
  },
  { immediate: true },
);
</script>

<template>
  <aside
    class="inspector-shell"
    :class="{ 'system-inspector-shell': store.workspaceSection !== 'library' }"
    :tabindex="store.workspaceSection === 'library' ? undefined : -1"
    :aria-labelledby="store.workspaceSection === 'library' ? undefined : systemPanelTitleId"
  >
    <section v-if="store.workspaceSection === 'library'" class="inspector-panel">
      <template v-if="store.activeWallpaper">
        <div class="inspector-kicker">
          <span>Selected</span>
          <IconButton icon="x" label="Close inspector" variant="quiet" @click="closeInspector" />
        </div>
        <div
          class="inspector-preview"
          :style="{ backgroundImage: store.activePreviewUrl ? `url(${store.activePreviewUrl})` : undefined }"
        >
          <button
            class="preview-favorite"
            :class="{ active: store.activeWallpaper.rating === 1 }"
            type="button"
            :aria-label="store.activeWallpaper.rating === 1 ? 'Unlike selected wallpaper' : 'Like selected wallpaper'"
            @click="store.activeWallpaper.rating === 1 ? store.resetRating(store.activeWallpaper.path) : store.like(store.activeWallpaper.path)"
          >
            <AppIcon name="heart" />
          </button>
        </div>

        <div class="inspector-title-row">
          <label class="wallpaper-title-field">
            <span>Title</span>
            <input
              v-model="titleDraft"
              type="text"
              maxlength="120"
              placeholder="Add title"
              :disabled="titleStatus === 'saving'"
              @blur="() => saveDisplayTitle()"
              @keydown.enter.prevent="saveDisplayTitle(true)"
            />
          </label>
          <div class="inspector-subtitle">
            {{ store.activeWallpaper.play_count }} plays - {{ formatDate(store.activeWallpaper.last_played) }}
            <span v-if="titleStatus === 'saving'"> - Saving</span>
            <span v-else-if="titleStatus === 'saved'"> - Saved</span>
          </div>
        </div>

        <div class="inspector-action-row">
          <button type="button" class="mini-action primary" @click="store.setAsWallpaper(store.activeWallpaper.path)">
            <AppIcon name="wallpaper" />
            <span>Set Wallpaper</span>
          </button>
          <button type="button" class="mini-action" @click="openContainingFolder(store.activeWallpaper.path)">
            <AppIcon name="folder" />
            <span>Open Folder</span>
          </button>
          <button type="button" class="mini-action" @click="copyPath(store.activeWallpaper.path)">
            <AppIcon :name="copiedPath ? 'check' : 'copy'" />
            <span>{{ copiedPath ? "Copied" : "Copy Path" }}</span>
          </button>
        </div>

        <div class="metadata-list">
          <MetadataRow
            icon="resolution"
            label="Resolution"
            :value="store.activeMetadata ? `${store.activeMetadata.width}x${store.activeMetadata.height}` : 'Unknown'"
          />
          <MetadataRow icon="file-size" label="File size" :value="formatBytes(store.activeMetadata?.file_size)" />
          <MetadataRow icon="calendar" label="Added" :value="formatDate(store.activeWallpaper.created_at)" />
          <MetadataRow icon="folder" label="Folder" :value="folderName(store.activeWallpaper.path)" :title="folderName(store.activeWallpaper.path)" />
          <MetadataRow
            v-if="store.activeShellMetadata?.authors"
            icon="author"
            label="Author"
            :value="store.activeShellMetadata.authors"
            :title="store.activeShellMetadata.authors"
          />
          <MetadataRow
            v-if="store.activeShellMetadata?.copyright"
            icon="copyright"
            label="Copyright"
            :value="store.activeShellMetadata.copyright"
            :title="store.activeShellMetadata.copyright"
          />
          <MetadataRow
            v-if="store.activeShellMetadata?.comment"
            icon="comment"
            label="Comment"
            :value="store.activeShellMetadata.comment"
            :title="store.activeShellMetadata.comment"
          />
        </div>

        <div class="inspector-label">Tags</div>
        <div class="tag-cloud">
          <button
            v-for="tag in store.activeWallpaper.tags"
            :key="tag.id"
            class="tag-pill-button"
            type="button"
            title="Remove tag"
            :aria-label="`Remove tag ${tag.name}`"
            @click="store.unassignTag(store.activeWallpaper.path, tag.id)"
          >
            <TagPill :label="tag.name" :color="tag.color" active removable />
          </button>
          <TagPill v-if="store.activeWallpaper.tags.length === 0" label="No tags" />
        </div>

        <TagCombobox />
      </template>
      <div v-else class="muted-note">No wallpaper selected</div>
    </section>

    <section v-else-if="store.workspaceSection === 'displays'" class="inspector-panel system-panel">
      <div class="system-panel__header">
        <AppIcon name="display" />
        <div>
          <h2 :id="systemPanelTitleId" class="system-panel__title">Displays</h2>
          <div class="system-panel__subtitle">{{ displaySummary }}</div>
        </div>
        <IconButton icon="x" label="Close displays" variant="quiet" @click="closePanel" />
      </div>

      <div class="segmented-control display-mode-switch" aria-label="Display mode">
        <button
          v-for="mode in displayModeOptions"
          :key="mode.key"
          type="button"
          :class="{ active: store.displayMode === mode.key }"
          :aria-pressed="store.displayMode === mode.key"
          :disabled="store.isChangingDisplayMode"
          @click="setDisplayMode(mode.key)"
        >
          {{ mode.label }}
        </button>
      </div>
      <p v-if="store.displayModeError" class="display-mode-error">{{ store.displayModeError }}</p>

      <div class="system-list">
        <div v-for="display in store.displays" :key="display.id" class="system-list-row">
          <AppIcon name="display" />
          <span>Display {{ display.index + 1 }}</span>
          <strong>{{ display.width }}x{{ display.height }}</strong>
        </div>
        <div v-if="store.displays.length === 0" class="muted-note compact">Display details unavailable</div>
      </div>
    </section>

    <section v-else-if="store.workspaceSection === 'settings'" class="inspector-panel system-panel">
      <div class="system-panel__header">
        <AppIcon name="settings" />
        <div>
          <h2 :id="systemPanelTitleId" class="system-panel__title">Settings</h2>
          <div class="system-panel__subtitle">Appearance and behavior</div>
        </div>
        <IconButton icon="x" label="Close settings" variant="quiet" @click="closePanel" />
      </div>

      <div class="setting-stack">
        <div class="theme-setting">
          <div class="theme-setting__header">
            <span>Theme</span>
            <strong>{{ theme === "system" ? "System" : theme === "dark" ? "Dark" : "Light" }}</strong>
          </div>
          <div class="segmented-control theme-segmented" aria-label="Application theme">
            <button
              type="button"
              :class="{ active: theme === 'dark' }"
              :aria-pressed="theme === 'dark'"
              @click="selectTheme('dark')"
            >
              <AppIcon name="theme-dark" />
              <span>Dark</span>
            </button>
            <button
              type="button"
              :class="{ active: theme === 'light' }"
              :aria-pressed="theme === 'light'"
              @click="selectTheme('light')"
            >
              <AppIcon name="theme-light" />
              <span>Light</span>
            </button>
            <button
              type="button"
              :class="{ active: theme === 'system' }"
              :aria-pressed="theme === 'system'"
              @click="selectTheme('system')"
            >
              <AppIcon name="desktop" />
              <span>System</span>
            </button>
          </div>
        </div>
        <div class="theme-setting">
          <div class="theme-setting__header">
            <span>Interface mode</span>
            <strong>{{ workspaceMode === "quiet" ? "Quiet Canvas" : "Workspace" }}</strong>
          </div>
          <div class="segmented-control theme-segmented" aria-label="Interface mode">
            <button
              type="button"
              :class="{ active: workspaceMode === 'workbench' }"
              :aria-pressed="workspaceMode === 'workbench'"
              @click="setWorkspaceMode('workbench')"
            >
              <AppIcon name="grid" />
              <span>Workspace</span>
            </button>
            <button
              type="button"
              :class="{ active: workspaceMode === 'quiet' }"
              :aria-pressed="workspaceMode === 'quiet'"
              @click="setWorkspaceMode('quiet')"
            >
              <AppIcon name="quiet-canvas" />
              <span>Quiet Canvas</span>
            </button>
          </div>
        </div>
        <LibrarySourcesSettings />
        <LibraryBackupSettings />
        <UpdateSettings />
        <div class="setting-card">
          <span>Focus Pause</span>
          <strong>{{ store.focusMode.enabled ? "On" : "Off" }}</strong>
          <button
            class="mini-action"
            type="button"
            :aria-pressed="store.focusMode.enabled"
            @click="store.setFocusModeEnabled(!store.focusMode.enabled)"
          >
            {{ store.focusMode.enabled ? "Disable" : "Enable" }}
          </button>
        </div>
        <div class="setting-card">
          <span>Fullscreen</span>
          <strong>{{ store.focusMode.fullscreen_detected ? "Detected" : "Clear" }}</strong>
        </div>
        <div class="setting-card">
          <span>Pause State</span>
          <strong>{{ store.isPaused ? "Paused" : "Running" }}</strong>
          <button class="mini-action" type="button" @click="store.togglePause">
            {{ store.isPaused ? "Resume" : "Pause" }}
          </button>
        </div>
      </div>
    </section>

    <section v-else-if="store.workspaceSection === 'shortcuts'" class="inspector-panel system-panel">
      <div class="system-panel__header">
        <AppIcon name="shortcuts" />
        <div>
          <h2 :id="systemPanelTitleId" class="system-panel__title">{{ COMMAND_LINE_TITLE }}</h2>
          <div class="system-panel__subtitle">{{ COMMAND_LINE_SUBTITLE }}</div>
        </div>
        <IconButton icon="x" label="Close command line" variant="quiet" @click="closePanel" />
      </div>
      <div class="system-list">
        <div v-for="row in commandLineRows" :key="row.label" class="system-list-row">
          <AppIcon name="chevron-right" />
          <span>{{ row.label }}</span>
          <strong>{{ row.value }}</strong>
        </div>
      </div>
    </section>

    <section v-else-if="store.workspaceSection === 'advanced'" class="inspector-panel system-panel">
      <div class="system-panel__header">
        <AppIcon name="advanced" />
        <div>
          <h2 :id="systemPanelTitleId" class="system-panel__title">Advanced</h2>
          <div class="system-panel__subtitle">Local desktop runtime</div>
        </div>
        <IconButton icon="x" label="Close advanced" variant="quiet" @click="closePanel" />
      </div>
      <div class="system-list">
        <div class="system-list-row">
          <AppIcon name="library" />
          <span>Framework</span>
          <strong>Tauri 2</strong>
        </div>
        <div class="system-list-row">
          <AppIcon name="storage" />
          <span>Database</span>
          <strong>SQLite</strong>
        </div>
        <div class="system-list-row">
          <AppIcon name="image" />
          <span>Thumbnails</span>
          <strong>Asset cache</strong>
        </div>
        <div class="setting-card diagnostic-card">
          <span>CLI diagnostics</span>
          <strong>{{ store.cliLog ? "Log available" : "No entries" }}</strong>
          <button
            class="mini-action"
            type="button"
            :disabled="store.isLoadingCliLog"
            @click="store.loadCliLog"
          >
            {{ store.isLoadingCliLog ? "Loading" : "Refresh" }}
          </button>
        </div>
      </div>
      <pre class="cli-log-viewer" aria-label="Recent CLI action failures">{{ store.cliLog || "No CLI action failures logged yet." }}</pre>
    </section>

    <section v-if="store.workspaceSection === 'insights'" class="inspector-panel insights-panel-compact">
      <div class="insights-header">
        <h2 :id="systemPanelTitleId" class="section-label">Yearly Insights {{ store.yearlyStats.year }}</h2>
        <IconButton
          v-if="store.workspaceSection === 'insights'"
          icon="x"
          label="Close insights"
          variant="quiet"
          @click="closePanel"
        />
      </div>
      <YearlyInsightsChart :months="store.yearlyStats.monthly" />
      <div class="insight-stat-list">
        <MetadataRow icon="library" label="Wallpapers Used" :value="store.yearlyStats.unique_wallpapers.toLocaleString()" />
        <MetadataRow icon="calendar" label="Total Time" :value="`${store.yearlyStats.total_plays.toLocaleString()} rotations`" />
        <MetadataRow icon="tag" label="Top Category" :value="topCategory" />
      </div>
    </section>
  </aside>
</template>
