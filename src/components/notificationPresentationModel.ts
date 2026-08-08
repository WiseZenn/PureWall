import type { NotificationTone } from "../composables/useNotifications";

export interface NotificationSemantics {
  role: "alert" | "status";
  live: "assertive" | "polite";
}

export function notificationSemantics(
  tone: NotificationTone,
): NotificationSemantics {
  return tone === "error"
    ? { role: "alert", live: "assertive" }
    : { role: "status", live: "polite" };
}
