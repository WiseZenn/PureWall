<script lang="ts">
export type WidgetPlaybackAction = "like" | "next" | "dislike";
export type WidgetCommandInvoker = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const FALLBACK_WIDGET_ERROR = "The action could not be completed. Please try again.";

export function getWidgetCommandErrorMessage(error: unknown) {
  const candidate = error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : error && typeof error === "object" && "message" in error && typeof error.message === "string"
        ? error.message
        : "";
  return candidate.trim() || FALLBACK_WIDGET_ERROR;
}

export function createWidgetCommandController(invokeCommand: WidgetCommandInvoker) {
  let errorMessage = "";
  return {
    get errorMessage() {
      return errorMessage;
    },
    async runPlaybackAction(action: WidgetPlaybackAction) {
      try {
        await invokeCommand("run_playback_action", { action });
        errorMessage = "";
      } catch (error) {
        errorMessage = getWidgetCommandErrorMessage(error);
      }
    },
    async hideWidget() {
      try {
        await invokeCommand("hide_widget");
        errorMessage = "";
      } catch (error) {
        errorMessage = getWidgetCommandErrorMessage(error);
      }
    },
  };
}
</script>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import AppIcon from "../components/AppIcon.vue";

const busy = ref(false);
const errorMessage = ref("");
const controller = createWidgetCommandController(invoke);

onMounted(() => {
  document.body.classList.add("widget-view");
});

onBeforeUnmount(() => {
  document.body.classList.remove("widget-view");
});

async function runPlaybackAction(action: WidgetPlaybackAction) {
  if (busy.value) return;
  busy.value = true;
  await controller.runPlaybackAction(action);
  errorMessage.value = controller.errorMessage;
  busy.value = false;
}

async function hideWidget() {
  if (busy.value) return;
  busy.value = true;
  await controller.hideWidget();
  errorMessage.value = controller.errorMessage;
  busy.value = false;
}
</script>

<template>
  <main class="widget-shell">
    <div class="widget-bar">
      <button class="widget-button" title="Like" aria-label="Like" :disabled="busy" @click="runPlaybackAction('like')">
        <AppIcon name="heart" />
      </button>
      <button class="widget-button widget-button-primary" title="Next" aria-label="Next" :disabled="busy" @click="runPlaybackAction('next')">
        <AppIcon name="next" />
        <span>Next</span>
      </button>
      <button class="widget-button" title="Dislike" aria-label="Dislike" :disabled="busy" @click="runPlaybackAction('dislike')">
        <AppIcon name="dislike" />
      </button>
      <div class="widget-separator" aria-hidden="true"></div>
      <button class="widget-button widget-button-close" title="Hide" aria-label="Hide" :disabled="busy" @click="hideWidget">
        <AppIcon name="x" />
      </button>
    </div>
    <p
      v-if="errorMessage"
      class="widget-error"
      role="status"
      style="position: fixed; right: 8px; bottom: 4px; left: 8px; margin: 0; padding: 2px 6px; border-radius: 6px; background: var(--bg-panel); color: var(--text-primary); font-size: 11px; line-height: 1.2; text-align: center;"
    >
      {{ errorMessage }}
    </p>
  </main>
</template>
