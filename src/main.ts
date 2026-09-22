import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { initializeTheme } from "./composables/useTheme";
import { initializeWorkspaceMode } from "./composables/useWorkspaceMode";
import "./fonts.css";
import "./styles.css";

initializeTheme();
initializeWorkspaceMode();

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.mount("#app");
