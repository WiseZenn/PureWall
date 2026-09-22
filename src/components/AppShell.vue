<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useWallpaperStore } from "../stores/wallpapers";
import EmptyState from "./EmptyState.vue";
import Gallery from "./Gallery.vue";
import InspectorPanel from "./InspectorPanel.vue";
import NotificationCenter from "./NotificationCenter.vue";
import QuietCanvas from "./QuietCanvas.vue";
import Sidebar from "./Sidebar.vue";
import StatusBar from "./StatusBar.vue";
import TitleBar from "./TitleBar.vue";
import { useWorkspaceMode } from "../composables/useWorkspaceMode";
import { useInspector } from "../composables/useInspector";
import { workspacePresentation } from "./workspacePresentation";

const store = useWallpaperStore();
const { workspaceMode } = useWorkspaceMode();
const { inspectorOpen } = useInspector();
const libraryTotal = computed(() => Math.max(store.stats.total, store.wallpaperTotal));
const workspaceState = computed(() =>
  workspacePresentation({ isLoading: store.isLoading, total: libraryTotal.value }),
);
const isDragOver = ref(false);
type DragDropUnlisten = () => void;
let unlistenDragDrop: DragDropUnlisten | null = null;
const showInspector = computed(
  () =>
    inspectorOpen.value &&
    (store.workspaceSection !== "library" ||
      libraryTotal.value > 0 ||
      store.isLoading),
);

onMounted(async () => {
  try {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        isDragOver.value = true;
        return;
      }
      if (payload.type === "leave") {
        isDragOver.value = false;
        return;
      }
      if (payload.type === "drop") {
        isDragOver.value = false;
        try {
          await store.importDroppedPaths(payload.paths);
        } catch {
          // The store reports the visible error notification.
        }
      }
    });
  } catch (error) {
    console.error("Failed to bind drag/drop import:", error);
  }
});

onBeforeUnmount(() => {
  unlistenDragDrop?.();
  unlistenDragDrop = null;
});

</script>

<template>
  <Transition name="workspace-mode-fade" mode="out-in">
    <QuietCanvas v-if="workspaceMode === 'quiet' && libraryTotal > 0" key="quiet-canvas" />
    <div
      v-else
      key="workbench"
      class="app-shell living-gallery-shell"
      :class="{ 'inspector-is-open': showInspector }"
    >
      <h1 id="app-main-heading" class="sr-only">PureWall wallpaper manager</h1>
      <a class="skip-link" href="#main-workspace">Skip to main content</a>
      <TitleBar />
      <div class="workbench-grid">
        <Sidebar />
        <main id="main-workspace" class="main-workspace" tabindex="-1" aria-labelledby="app-main-heading">
          <section
            v-if="workspaceState === 'loading'"
            class="workspace-state workspace-state--loading"
            role="status"
            aria-live="polite"
            aria-busy="true"
          >
            <div class="spinner"></div>
            <div class="workspace-state__copy">
              <h2>Loading your library</h2>
              <p>Reading your local wallpaper metadata.</p>
            </div>
          </section>
          <EmptyState v-else-if="workspaceState === 'empty'" />
          <Gallery v-else />
        </main>
        <InspectorPanel v-if="showInspector" />
      </div>
      <StatusBar />
      <NotificationCenter />
    </div>
  </Transition>
  <div v-if="isDragOver" class="drop-import-overlay" role="status" aria-live="polite">
    <div>
      <strong>Drop to import</strong>
      <span>Images and folders will be added to your collection</span>
    </div>
  </div>
</template>
