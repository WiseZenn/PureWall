import { computed, ref } from "vue";

export type NotificationTone = "error" | "info" | "success";

export interface AppNotification {
  id: number;
  title: string;
  message: string;
  tone: NotificationTone;
  actionLabel?: string;
  action?: () => void | Promise<void>;
}

const notifications = ref<AppNotification[]>([]);
let nextId = 1;

function notify(
  title: string,
  message: string,
  tone: NotificationTone = "info",
  action?: { label: string; run: () => void | Promise<void> },
) {
  const id = nextId++;
  notifications.value = [
    ...notifications.value,
    { id, title, message, tone, actionLabel: action?.label, action: action?.run },
  ];
  window.setTimeout(() => dismiss(id), tone === "error" ? 7000 : action ? 8000 : 4000);
  return id;
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }

  return "The operation could not be completed.";
}

function notifyError(title: string, error: unknown) {
  notify(title, getErrorMessage(error), "error");
}

function dismiss(id: number) {
  notifications.value = notifications.value.filter((notification) => notification.id !== id);
}

function runAction(id: number) {
  const notification = notifications.value.find((item) => item.id === id);
  if (!notification?.action) return;
  void notification.action();
  dismiss(id);
}

export function useNotifications() {
  return {
    notifications: computed(() => notifications.value),
    notify,
    notifyError,
    dismiss,
    runAction,
  };
}
