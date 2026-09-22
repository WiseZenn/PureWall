import { createSSRApp } from "vue";
import { renderToString } from "@vue/server-renderer";
import { describe, expect, it, vi } from "vitest";
import NotificationCenter from "./NotificationCenter.vue";

vi.mock("../composables/useNotifications", async () => {
  const { ref } = await import("vue");

  return {
    useNotifications: () => ({
      notifications: ref([
        { id: 1, title: "Info", message: "Info message", tone: "info" },
        { id: 2, title: "Success", message: "Success message", tone: "success" },
        { id: 3, title: "Error", message: "Error message", tone: "error" },
      ]),
      dismiss: () => undefined,
      runAction: () => undefined,
    }),
  };
});

describe("NotificationCenter live-region semantics", () => {
  it.each([
    ["info", "status", "polite"],
    ["success", "status", "polite"],
    ["error", "alert", "assertive"],
  ] as const)(
    "renders %s notifications with role=%s and aria-live=%s",
    async (tone, role, live) => {
      const html = await renderToString(createSSRApp(NotificationCenter));
      const notification = html.match(
        new RegExp(
          '<div[^>]*class="[^"]*app-notification--' + tone + '[^"]*"[^>]*>',
        ),
      )?.[0];

      expect(notification).toBeDefined();
      expect(notification).toContain('role="' + role + '"');
      expect(notification).toContain('aria-live="' + live + '"');
      expect(notification).toContain('aria-atomic="true"');
    },
  );
});
