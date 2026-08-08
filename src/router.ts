import { createRouter, createWebHashHistory } from "vue-router";
import Home from "./views/Home.vue";
import WidgetView from "./views/WidgetView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", component: Home },
    { path: "/widget", component: WidgetView },
  ],
});
