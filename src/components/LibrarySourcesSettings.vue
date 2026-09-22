<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useWallpaperStore } from "../stores/wallpapers";
import type {
  LibrarySource,
  RemoveLibrarySourceImpact,
  RemoveLibrarySourceMode,
} from "../stores/wallpapers";
import AppIcon from "./AppIcon.vue";
import { primarySourceAction, removeSourceCopy } from "./librarySourceModel";

const store = useWallpaperStore();
const removeCandidate = ref<LibrarySource | null>(null);
const removeImpact = ref<RemoveLibrarySourceImpact | null>(null);
const removeLoading = ref(false);
const panelError = ref("");
const addButton = ref<HTMLButtonElement | null>(null);
const removeDialog = ref<HTMLElement | null>(null);
const removeCloseButton = ref<HTMLButtonElement | null>(null);
const removeTrigger = ref<HTMLElement | null>(null);

const removeCopy = computed(() =>
  removeSourceCopy(removeImpact.value?.affected_wallpapers ?? 0),
);
const removeBusy = computed(() => {
  const source = removeCandidate.value;
  return source ? store.librarySourceBusy.has(source.path) : false;
});
const removeError = computed(() => {
  const source = removeCandidate.value;
  return source ? store.librarySourceErrors.get(source.path) ?? "" : "";
});

function sourceKindLabel(source: LibrarySource): string {
  return source.source === "mounted" ? "Gallery folder" : "Imported folder";
}

function statusLabel(source: LibrarySource): string {
  return source.status.charAt(0).toUpperCase() + source.status.slice(1);
}

function formatLastScan(value: string | null): string {
  if (!value) return "Not scanned yet";
  const parsed = new Date(value.replace(" ", "T"));
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

function rowError(source: LibrarySource): string {
  return store.librarySourceErrors.get(source.path) ?? source.last_error ?? "";
}

function rowBusy(source: LibrarySource): boolean {
  return store.librarySourceBusy.has(source.path);
}

function actionLabel(source: LibrarySource): "Rescan" | "Retry" {
  return primarySourceAction(source.status) === "retry" ? "Retry" : "Rescan";
}

function busyLabel(source: LibrarySource): string {
  const operation = store.librarySourceBusy.get(source.path);
  if (!operation) return "";
  return operation === "rescan"
    ? "Scanning"
    : operation.charAt(0).toUpperCase() + operation.slice(1);
}

async function addSource() {
  panelError.value = "";
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    await store.importFolder(selected);
  } catch {
    // The store owns command notifications; keep this surface ready for retry.
  }
}

async function runPrimaryAction(source: LibrarySource) {
  try {
    if (primarySourceAction(source.status) === "retry") {
      await store.retryLibrarySource(source.path);
    } else {
      await store.rescanLibrarySource(source.path);
    }
  } catch {
    // The store retains the actionable row error and visible notification.
  }
}

async function relocate(source: LibrarySource) {
  panelError.value = "";
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    try {
      await store.relocateLibrarySource(source.path, selected);
    } catch {
      // The store retains the actionable row error and visible notification.
    }
  } catch (error) {
    console.error("Failed to open relocate folder picker:", error);
    panelError.value = "Could not open the folder picker. Try again.";
  }
}

async function prepareRemove(source: LibrarySource) {
  panelError.value = "";
  removeTrigger.value =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
  removeCandidate.value = source;
  removeImpact.value = null;
  removeLoading.value = true;
  await nextTick();
  removeDialog.value?.focus();
  try {
    removeImpact.value = await store.previewRemoveLibrarySource(source.path);
    await nextTick();
    removeCloseButton.value?.focus();
  } catch (error) {
    console.error("Failed to preview library source removal:", error);
    panelError.value = "Could not prepare source removal. Try again.";
    await closeRemoveSurface();
  } finally {
    removeLoading.value = false;
  }
}

async function confirmRemove(mode: RemoveLibrarySourceMode) {
  const source = removeCandidate.value;
  if (!source) return;
  try {
    await store.removeLibrarySource(source.path, mode);
    await closeRemoveSurface();
  } catch {
    // The dialog stays open; the store retains the row error.
  }
}

async function closeRemoveSurface() {
  const trigger = removeTrigger.value;
  removeTrigger.value = null;
  removeCandidate.value = null;
  removeImpact.value = null;
  await nextTick();
  if (trigger?.isConnected) {
    trigger.focus();
  } else {
    addButton.value?.focus();
  }
}

function cancelRemove() {
  if (removeLoading.value || removeBusy.value) return;
  void closeRemoveSurface();
}

function handleKeydown(event: KeyboardEvent) {
  if (!removeCandidate.value) return;
  if (event.key === "Escape") {
    cancelRemove();
    return;
  }
  if (event.key !== "Tab") return;

  const dialog = removeDialog.value;
  if (!dialog) return;
  const controls = Array.from(
    dialog.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  );
  if (controls.length === 0) {
    event.preventDefault();
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
  void store.loadLibrarySources();
  window.addEventListener("keydown", handleKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <section class="library-sources-setting" aria-labelledby="library-sources-title">
    <div class="library-sources-setting__header">
      <div>
        <span id="library-sources-title">Library sources</span>
        <small>Folders PureWall watches for wallpapers</small>
      </div>
      <button
        ref="addButton"
        class="mini-action primary"
        type="button"
        :disabled="store.isImporting"
        @click="addSource"
      >
        <AppIcon name="plus" />
        <span>{{ store.isImporting ? "Adding" : "Add" }}</span>
      </button>
    </div>

    <p v-if="panelError" class="library-sources-setting__error" role="alert">
      {{ panelError }}
    </p>

    <div v-if="store.librarySources.length > 0" class="source-list">
      <article
        v-for="source in store.librarySources"
        :key="source.path"
        class="source-row"
        :aria-busy="rowBusy(source)"
      >
        <div class="source-row__top">
          <div class="source-row__identity">
            <strong :title="source.path">{{ source.path }}</strong>
            <span>{{ sourceKindLabel(source) }}</span>
          </div>
          <span :class="['source-status', `source-status--${source.status}`]">
            <i aria-hidden="true" />
            {{ statusLabel(source) }}
          </span>
        </div>

        <div class="source-row__counts" aria-label="Wallpaper availability">
          <span><strong>{{ source.available_count.toLocaleString() }}</strong> available</span>
          <span><strong>{{ source.unavailable_count.toLocaleString() }}</strong> unavailable</span>
        </div>

        <p class="source-row__scan">Last scan: {{ formatLastScan(source.last_scan_at) }}</p>
        <p v-if="rowError(source)" class="source-row__error" role="alert">
          {{ rowError(source) }}
        </p>

        <div class="source-row__actions">
          <button
            class="mini-action primary"
            type="button"
            :disabled="rowBusy(source)"
            @click="runPrimaryAction(source)"
          >
            {{ rowBusy(source) ? busyLabel(source) : actionLabel(source) }}
          </button>
          <button
            class="mini-action"
            type="button"
            :disabled="rowBusy(source)"
            @click="relocate(source)"
          >
            Relocate
          </button>
          <button
            class="mini-action"
            type="button"
            :disabled="rowBusy(source)"
            @click="prepareRemove(source)"
          >
            Remove
          </button>
        </div>
      </article>
    </div>

    <div v-else class="source-empty" aria-live="polite">
      <AppIcon name="folder" />
      <span>No library sources yet</span>
      <small>Add a folder to start watching it.</small>
    </div>
  </section>

  <Teleport to="body">
    <div
      v-if="removeCandidate"
      class="remove-source-backdrop"
      @click.self="cancelRemove"
    >
      <section
        ref="removeDialog"
        class="remove-source-dialog"
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        :aria-busy="removeLoading || removeBusy"
        aria-labelledby="remove-source-title"
        aria-describedby="remove-source-safety"
      >
        <div class="remove-source-dialog__header">
          <div>
            <span class="remove-source-dialog__eyebrow">Library source</span>
            <h2 id="remove-source-title">Remove this folder?</h2>
          </div>
          <button
            ref="removeCloseButton"
            class="remove-source-dialog__close"
            type="button"
            aria-label="Close remove source dialog"
            :disabled="removeLoading || removeBusy"
            @click="cancelRemove"
          >
            <AppIcon name="x" />
          </button>
        </div>

        <p class="remove-source-dialog__path" :title="removeCandidate.path">
          {{ removeCandidate.path }}
        </p>

        <div v-if="removeLoading" class="remove-source-dialog__loading" aria-live="polite">
          Checking affected wallpapers…
        </div>

        <template v-else-if="removeImpact">
          <div class="remove-source-options">
            <div>
              <strong>Keep metadata</strong>
              <p>{{ removeCopy.keepMetadata }}</p>
            </div>
            <div>
              <strong>Clear metadata</strong>
              <p>{{ removeCopy.clearMetadata }}</p>
            </div>
          </div>

          <div id="remove-source-safety" class="remove-source-safety">
            <AppIcon name="shield" />
            <span>{{ removeCopy.safety }}</span>
          </div>

          <p v-if="removeError" class="remove-source-dialog__error" role="alert">
            {{ removeError }}
          </p>

          <div class="remove-source-dialog__actions">
            <button
              class="mini-action"
              type="button"
              :disabled="removeBusy"
              @click="cancelRemove"
            >
              Cancel
            </button>
            <button
              class="mini-action"
              type="button"
              :disabled="removeBusy"
              @click="confirmRemove('clear_metadata')"
            >
              Clear metadata
            </button>
            <button
              class="mini-action primary"
              type="button"
              :disabled="removeBusy"
              @click="confirmRemove('keep_metadata')"
            >
              {{ removeBusy ? "Removing" : "Keep metadata" }}
            </button>
          </div>
        </template>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.library-sources-setting {
  display: grid;
  gap: var(--space-3);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.library-sources-setting__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.library-sources-setting__header > div {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.library-sources-setting__header > div > span {
  color: var(--text-primary);
  font-size: 12px;
  font-weight: 620;
}

.library-sources-setting__header small,
.source-empty small {
  color: var(--text-muted);
  font-size: 10px;
}

.library-sources-setting__error,
.source-row__error,
.remove-source-dialog__error {
  padding: var(--space-2);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: 11px;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.source-list {
  display: grid;
  gap: var(--space-2);
}

.source-row {
  min-width: 0;
  display: grid;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-panel-elevated);
  transition: border-color 0.18s ease, background 0.18s ease;
}

.source-row:hover {
  border-color: var(--border-strong);
}

.source-row__top {
  min-width: 0;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-2);
}

.source-row__identity {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.source-row__identity strong {
  color: var(--text-primary);
  font-size: 11px;
  font-weight: 580;
  overflow-wrap: anywhere;
  white-space: normal;
}

.source-row__identity span,
.source-row__scan {
  color: var(--text-muted);
  font-size: 10px;
}

.source-status {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--text-secondary);
  font-size: 10px;
  font-weight: 620;
}

.source-status i {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--text-muted);
}

.source-status--online i,
.source-status--scanning i {
  background: var(--accent);
}

.source-status--scanning i {
  animation: source-status-pulse 1.2s ease-in-out infinite;
}

.source-status--error {
  color: var(--text-primary);
}

.source-row__counts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.source-row__counts span {
  min-width: 0;
  padding: 5px 7px;
  border-radius: var(--radius-sm);
  background: var(--bg-hover);
  color: var(--text-secondary);
  font-size: 10px;
}

.source-row__counts strong {
  color: var(--text-primary);
  font-weight: 620;
}

.source-row__actions {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-1);
}

.source-row__actions .mini-action {
  padding-inline: 5px;
  font-size: 10px;
}

.source-empty {
  min-height: 92px;
  display: grid;
  place-items: center;
  align-content: center;
  gap: var(--space-1);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  text-align: center;
}

.source-empty .app-icon {
  width: 20px;
  height: 20px;
  color: var(--accent);
}

.remove-source-backdrop {
  position: fixed;
  z-index: 5000;
  inset: 0;
  display: grid;
  place-items: center;
  padding: var(--space-4);
  background: rgba(4, 8, 12, 0.68);
}

.remove-source-dialog {
  width: min(460px, calc(100vw - 32px));
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

.remove-source-dialog__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.remove-source-dialog__eyebrow {
  display: block;
  margin-bottom: 3px;
  color: var(--accent);
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.remove-source-dialog h2 {
  color: var(--text-primary);
  font-size: 17px;
  font-weight: 660;
}

.remove-source-dialog__close {
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

.remove-source-dialog__close:hover {
  border-color: var(--border-subtle);
  background: var(--bg-hover);
  color: var(--text-primary);
}

.remove-source-dialog__path {
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  background: var(--bg-panel-elevated);
  color: var(--text-secondary);
  font-family: "Cascadia Mono", Consolas, monospace;
  font-size: 11px;
  overflow-wrap: anywhere;
  white-space: normal;
}

.remove-source-dialog__loading {
  min-height: 96px;
  display: grid;
  place-items: center;
  color: var(--text-secondary);
}

.remove-source-options {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.remove-source-options > div {
  display: grid;
  align-content: start;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-panel-elevated);
}

.remove-source-options strong {
  color: var(--text-primary);
  font-size: 12px;
}

.remove-source-options p {
  color: var(--text-secondary);
  font-size: 11px;
  line-height: 1.5;
}

.remove-source-safety {
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

.remove-source-safety .app-icon {
  width: 18px;
  height: 18px;
  color: var(--accent);
}

.remove-source-dialog__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-2);
}

@keyframes source-status-pulse {
  50% {
    opacity: 0.35;
  }
}

@media (max-width: 520px) {
  .remove-source-options {
    grid-template-columns: 1fr;
  }

  .remove-source-dialog__actions {
    display: grid;
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  .source-row,
  .remove-source-dialog__close {
    transition: none;
  }

  .source-status--scanning i {
    animation: none;
  }
}
</style>
