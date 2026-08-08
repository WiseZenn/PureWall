<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useTheme, type ThemeMode } from "../composables/useTheme";
import {
  useWorkspaceMode,
  type WorkspaceMode,
} from "../composables/useWorkspaceMode";
import {
  useWallpaperStore,
  type BackupImportPreview,
  type BackupImportResult,
} from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";
import {
  backupImportPresentation,
  backupPreviewRows,
  missingPathMessage,
} from "./backupPresentationModel";

const JSON_FILTER = [
  {
    name: "PureWall backup",
    extensions: ["json"],
  },
];

const store = useWallpaperStore();
const { theme, setTheme } = useTheme();
const { workspaceMode, setWorkspaceMode } = useWorkspaceMode();
const panelError = ref("");
const dialogError = ref("");
const selectedPath = ref("");
const preview = ref<BackupImportPreview | null>(null);
const importResult = ref<BackupImportResult | null>(null);
const dialogOpen = ref(false);
const previewLoading = ref(false);
const activeAction = ref<"export" | "preview" | "import" | null>(null);
const importButton = ref<HTMLButtonElement | null>(null);
const backupDialog = ref<HTMLElement | null>(null);
const dialogCloseButton = ref<HTMLButtonElement | null>(null);
const dialogTrigger = ref<HTMLElement | null>(null);

const dialogBusy = computed(
  () => previewLoading.value || store.isBackupBusy,
);
const previewRows = computed(() =>
  preview.value ? backupPreviewRows(preview.value) : [],
);
const importPresentation = computed(() =>
  importResult.value ? backupImportPresentation(importResult.value) : null,
);
const visibleDialogError = computed(
  () => dialogError.value || store.backupError,
);
const dialogDescriptionId = computed(() => {
  if (importResult.value) return "backup-result-safety";
  if (preview.value) return "backup-preview-safety";
  return undefined;
});

function isThemeMode(value: string | null): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark";
}

function isWorkspaceMode(value: string | null): value is WorkspaceMode {
  return value === "workbench" || value === "quiet";
}

async function exportBackup() {
  panelError.value = "";
  try {
    const destination = await save({
      defaultPath: "purewall-backup-v1.json",
      filters: JSON_FILTER,
    });
    if (typeof destination !== "string") return;

    activeAction.value = "export";
    await store.exportLibraryBackup(destination, {
      theme: theme.value,
      workspaceMode: workspaceMode.value,
    });
  } catch (error) {
    if (!store.backupError) {
      console.error("Failed to open backup destination picker:", error);
      panelError.value = "Could not open the backup destination. Try again.";
    }
  } finally {
    activeAction.value = null;
  }
}

async function chooseBackup() {
  panelError.value = "";
  dialogError.value = "";
  dialogTrigger.value =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : importButton.value;

  let selected: string | string[] | null;
  try {
    selected = await open({
      directory: false,
      multiple: false,
      filters: JSON_FILTER,
    });
  } catch (error) {
    console.error("Failed to open backup file picker:", error);
    panelError.value = "Could not open the backup file picker. Try again.";
    dialogTrigger.value = null;
    return;
  }

  if (typeof selected !== "string") {
    dialogTrigger.value = null;
    return;
  }

  selectedPath.value = selected;
  preview.value = null;
  importResult.value = null;
  dialogOpen.value = true;
  previewLoading.value = true;
  activeAction.value = "preview";
  await nextTick();
  backupDialog.value?.focus();

  try {
    preview.value = await store.previewBackupImport(selected);
  } catch {
    dialogError.value =
      store.backupError || "Could not preview this backup. Try another file.";
  } finally {
    previewLoading.value = false;
    activeAction.value = null;
    await nextTick();
    dialogCloseButton.value?.focus();
  }
}

async function confirmImport() {
  if (!preview.value || !selectedPath.value) return;
  dialogError.value = "";
  activeAction.value = "import";

  try {
    const result = await store.importLibraryBackup(
      selectedPath.value,
      preview.value.contentDigest,
    );
    importResult.value = result;

    const { clientSettings } = result.merge;
    if (isThemeMode(clientSettings.theme)) {
      setTheme(clientSettings.theme);
    }
    if (isWorkspaceMode(clientSettings.workspaceMode)) {
      setWorkspaceMode(clientSettings.workspaceMode);
    }
    await nextTick();
    dialogCloseButton.value?.focus();
  } catch {
    dialogError.value =
      store.backupError || "Could not import this backup. No changes were made.";
  } finally {
    activeAction.value = null;
  }
}

async function closeDialog() {
  if (dialogBusy.value) return;
  const trigger = dialogTrigger.value;
  dialogTrigger.value = null;
  dialogOpen.value = false;
  selectedPath.value = "";
  preview.value = null;
  importResult.value = null;
  dialogError.value = "";
  await nextTick();

  if (trigger?.isConnected) {
    trigger.focus();
  } else {
    importButton.value?.focus();
  }
}

function cancelDialog() {
  if (dialogBusy.value) return;
  void closeDialog();
}

function handleKeydown(event: KeyboardEvent) {
  if (!dialogOpen.value) return;
  if (event.key === "Escape") {
    cancelDialog();
    return;
  }
  if (event.key !== "Tab") return;

  const dialog = backupDialog.value;
  if (!dialog) return;
  const controls = Array.from(
    dialog.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  );
  if (controls.length === 0) {
    event.preventDefault();
    dialog.focus();
    return;
  }

  const first = controls[0];
  const last = controls[controls.length - 1];
  const active = document.activeElement;
  if (
    event.shiftKey &&
    (active === dialog || active === first || !dialog.contains(active))
  ) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <section class="backup-setting" aria-labelledby="backup-setting-title">
    <div class="backup-setting__header">
      <div>
        <span id="backup-setting-title">Library backup</span>
        <small>Portable metadata for your existing wallpaper files</small>
      </div>
    </div>

    <div class="backup-setting__actions">
      <button
        class="mini-action"
        type="button"
        :disabled="store.isBackupBusy"
        @click="exportBackup"
      >
        <AppIcon name="download" />
        <span>{{ activeAction === "export" ? "Exporting" : "Export" }}</span>
      </button>
      <button
        ref="importButton"
        class="mini-action primary"
        type="button"
        :disabled="store.isBackupBusy"
        @click="chooseBackup"
      >
        <AppIcon name="upload" />
        <span>{{ activeAction === "preview" ? "Checking" : "Import" }}</span>
      </button>
    </div>

    <div class="backup-setting__safety">
      <AppIcon name="shield" />
      <span>Metadata only. Original wallpaper files are never copied or changed.</span>
    </div>

    <p
      v-if="panelError || (!dialogOpen && store.backupError)"
      class="backup-setting__error"
      role="alert"
    >
      {{ panelError || store.backupError }}
    </p>
  </section>

  <Teleport to="body">
    <div
      v-if="dialogOpen"
      class="backup-dialog-backdrop"
      @click.self="cancelDialog"
    >
      <section
        ref="backupDialog"
        class="backup-dialog"
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        :aria-busy="dialogBusy"
        aria-labelledby="backup-dialog-title"
        :aria-describedby="dialogDescriptionId"
      >
        <div class="backup-dialog__header">
          <div>
            <span class="backup-dialog__eyebrow">Library backup</span>
            <h2 id="backup-dialog-title">
              {{ importResult ? "Metadata import complete" : "Preview backup import" }}
            </h2>
          </div>
          <button
            ref="dialogCloseButton"
            class="backup-dialog__close"
            type="button"
            aria-label="Close backup dialog"
            :disabled="dialogBusy"
            @click="cancelDialog"
          >
            <AppIcon name="x" />
          </button>
        </div>

        <p class="backup-dialog__path" :title="selectedPath">
          {{ selectedPath }}
        </p>

        <div
          v-if="previewLoading"
          class="backup-dialog__loading"
          aria-live="polite"
        >
          Checking backup metadata…
        </div>

        <template v-else-if="importPresentation">
          <div class="backup-result" aria-live="polite">
            <AppIcon name="check" />
            <div>
              <strong>{{ importPresentation.title }}</strong>
              <p>{{ importPresentation.summary }}</p>
            </div>
          </div>

          <ul class="backup-result__details" aria-label="Committed changes">
            <li v-for="detail in importPresentation.details" :key="detail">
              {{ detail }}
            </li>
          </ul>

          <div id="backup-result-safety" class="backup-dialog__safety">
            <AppIcon name="shield" />
            <span>Original wallpaper files were not restored or modified.</span>
          </div>

          <div
            v-if="importPresentation.warnings.length"
            class="backup-dialog__warnings"
            aria-label="Post-import warnings"
          >
            <strong>Metadata was committed with warnings</strong>
            <ul>
              <li v-for="warning in importPresentation.warnings" :key="warning">
                {{ warning }}
              </li>
            </ul>
          </div>

          <div class="backup-dialog__actions">
            <button
              class="mini-action primary"
              type="button"
              @click="closeDialog"
            >
              Done
            </button>
          </div>
        </template>

        <template v-else-if="preview">
          <div class="backup-summary" aria-label="Backup contents">
            <div v-for="row in previewRows" :key="row.label">
              <span>{{ row.label }}</span>
              <strong>{{ row.value }}</strong>
            </div>
          </div>

          <p class="backup-dialog__availability">
            {{ missingPathMessage(preview.missingPaths) }}
          </p>

          <div id="backup-preview-safety" class="backup-dialog__safety">
            <AppIcon name="shield" />
            <span>
              Import merges metadata into this library. It never copies or changes
              wallpaper files.
            </span>
          </div>

          <div
            v-if="preview.warnings.length"
            class="backup-dialog__warnings"
            aria-label="Preview warnings"
          >
            <strong>Review before importing</strong>
            <ul>
              <li v-for="warning in preview.warnings" :key="warning">
                {{ warning }}
              </li>
            </ul>
          </div>

          <p v-if="visibleDialogError" class="backup-dialog__error" role="alert">
            {{ visibleDialogError }}
          </p>

          <div class="backup-dialog__actions">
            <button
              class="mini-action"
              type="button"
              :disabled="dialogBusy"
              @click="cancelDialog"
            >
              Cancel
            </button>
            <button
              class="mini-action primary"
              type="button"
              :disabled="dialogBusy || !preview"
              @click="confirmImport"
            >
              {{ activeAction === "import" ? "Importing" : "Import backup" }}
            </button>
          </div>
        </template>

        <template v-else>
          <p v-if="visibleDialogError" class="backup-dialog__error" role="alert">
            {{ visibleDialogError }}
          </p>
          <div v-if="visibleDialogError" class="backup-dialog__actions">
            <button class="mini-action primary" type="button" @click="closeDialog">
              Close
            </button>
          </div>
        </template>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.backup-setting {
  display: grid;
  gap: var(--space-3);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.backup-setting__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.backup-setting__header > div {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.backup-setting__header span {
  color: var(--text-primary);
  font-size: 12px;
  font-weight: 620;
}

.backup-setting__header small {
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1.4;
}

.backup-setting__actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.backup-setting__safety,
.backup-dialog__safety {
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr);
  align-items: start;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--accent-border);
  border-radius: var(--radius-md);
  background: var(--accent-soft);
  color: var(--text-primary);
  font-size: 11px;
  line-height: 1.45;
}

.backup-setting__safety .app-icon,
.backup-dialog__safety .app-icon {
  width: 18px;
  height: 18px;
  color: var(--accent);
}

.backup-setting__error,
.backup-dialog__error {
  padding: var(--space-2);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: 11px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.backup-dialog-backdrop {
  position: fixed;
  z-index: 5000;
  inset: 0;
  display: grid;
  place-items: center;
  padding: var(--space-4);
  background: rgba(4, 8, 12, 0.68);
}

.backup-dialog {
  width: min(500px, calc(100vw - 32px));
  max-height: calc(100vh - 32px);
  display: grid;
  gap: var(--space-3);
  overflow-y: auto;
  padding: var(--space-4);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--bg-panel);
  box-shadow: var(--shadow-panel);
}

.backup-dialog__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.backup-dialog__eyebrow {
  display: block;
  margin-bottom: 3px;
  color: var(--accent);
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.backup-dialog h2 {
  color: var(--text-primary);
  font-size: 17px;
  font-weight: 660;
}

.backup-dialog__close {
  width: 30px;
  height: 30px;
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  transition: background 0.18s ease, border-color 0.18s ease, color 0.18s ease;
}

.backup-dialog__close:hover {
  border-color: var(--border-subtle);
  background: var(--bg-hover);
  color: var(--text-primary);
}

.backup-dialog__close:focus-visible,
.backup-dialog:focus-visible,
.backup-setting button:focus-visible,
.backup-dialog button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.backup-dialog__path {
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  background: var(--bg-panel-elevated);
  color: var(--text-secondary);
  font-family: "Cascadia Mono", Consolas, monospace;
  font-size: 11px;
  overflow-wrap: anywhere;
  white-space: normal;
}

.backup-dialog__loading {
  min-height: 112px;
  display: grid;
  place-items: center;
  color: var(--text-secondary);
  font-size: 12px;
}

.backup-summary {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.backup-summary > div {
  min-width: 0;
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-panel-elevated);
}

.backup-summary span {
  color: var(--text-secondary);
  font-size: 10px;
  line-height: 1.35;
}

.backup-summary strong {
  color: var(--text-primary);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.backup-dialog__availability {
  color: var(--text-secondary);
  font-size: 11px;
  line-height: 1.45;
}

.backup-result {
  display: grid;
  grid-template-columns: 22px minmax(0, 1fr);
  align-items: start;
  gap: var(--space-2);
}

.backup-result > .app-icon {
  width: 22px;
  height: 22px;
  color: var(--accent);
}

.backup-result div {
  display: grid;
  gap: 3px;
}

.backup-result strong {
  color: var(--text-primary);
  font-size: 13px;
}

.backup-result p,
.backup-result__details {
  color: var(--text-secondary);
  font-size: 11px;
  line-height: 1.5;
}

.backup-result__details,
.backup-dialog__warnings ul {
  margin: 0;
  padding-left: 18px;
}

.backup-result__details {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 3px var(--space-3);
}

.backup-dialog__warnings {
  display: grid;
  gap: var(--space-1);
  padding: var(--space-3);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--bg-hover);
  color: var(--text-secondary);
  font-size: 11px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.backup-dialog__warnings strong {
  color: var(--text-primary);
  font-size: 11px;
}

.backup-dialog__warnings ul {
  display: grid;
  gap: 3px;
}

.backup-dialog__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-2);
}

@media (max-width: 520px) {
  .backup-summary,
  .backup-result__details {
    grid-template-columns: 1fr;
  }

  .backup-dialog__actions {
    display: grid;
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  .backup-dialog__close {
    transition: none;
  }
}
</style>
