<script setup lang="ts">
import { useNotifications } from "../composables/useNotifications";
import { notificationSemantics } from "./notificationPresentationModel";

const { notifications, dismiss, runAction } = useNotifications();
</script>

<template>
  <div class="notification-region">
    <div
      v-for="notification in notifications"
      :key="notification.id"
      class="app-notification"
      :class="`app-notification--${notification.tone}`"
      :role="notificationSemantics(notification.tone).role"
      :aria-live="notificationSemantics(notification.tone).live"
      aria-atomic="true"
    >
      <div>
        <strong>{{ notification.title }}</strong>
        <p>{{ notification.message }}</p>
        <button
          v-if="notification.actionLabel"
          type="button"
          class="notification-action"
          @click="runAction(notification.id)"
        >
          {{ notification.actionLabel }}
        </button>
      </div>
      <button
        type="button"
        class="notification-dismiss"
        :aria-label="`Dismiss ${notification.title}`"
        @click="dismiss(notification.id)"
      >
        x
      </button>
    </div>
  </div>
</template>
