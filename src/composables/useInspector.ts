import { nextTick, ref } from "vue";

const inspectorOpen = ref(true);
let inspectorOpener: HTMLElement | null = null;

function connectedFocusTarget(target: HTMLElement | null) {
  return target?.isConnected && typeof target.focus === "function"
    ? target
    : null;
}

export function useInspector() {
  const openInspector = (opener?: HTMLElement | null) => {
    if (opener) inspectorOpener = opener;
    inspectorOpen.value = true;
  };

  const closeInspector = async () => {
    inspectorOpen.value = false;
    await nextTick();

    const fallback =
      typeof document === "undefined"
        ? null
        : document.getElementById("main-workspace");
    const target =
      connectedFocusTarget(inspectorOpener) ??
      connectedFocusTarget(fallback);
    inspectorOpener = null;
    target?.focus();
  };

  const toggleInspector = (opener?: HTMLElement | null) => {
    if (inspectorOpen.value) {
      if (opener) inspectorOpener = opener;
      void closeInspector();
    } else {
      openInspector(opener);
    }
  };

  return { inspectorOpen, openInspector, closeInspector, toggleInspector };
}
