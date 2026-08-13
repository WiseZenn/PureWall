export function captureReadinessSnapshot(document) {
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" && style.display !== "none";
  };
  const images = Array.from(document.images).filter(visible);
  const loadingSelectors = [
    ".workspace-state--loading",
    ".loading-state",
    ".gallery-page-loading",
    ".tile-loading",
    ".living-stage__placeholder",
    '[aria-busy="true"]',
  ];
  const loading = loadingSelectors.flatMap((selector) => Array.from(document.querySelectorAll(selector)).filter(visible));
  const geometry = Array.from(document.querySelectorAll("body *")).filter(visible).map((element) => {
    const rect = element.getBoundingClientRect();
    return `${element.tagName}:${Math.round(rect.left)},${Math.round(rect.top)},${Math.round(rect.width)},${Math.round(rect.height)}`;
  }).join("|");
  return {
    imageCount: images.length,
    imagesReady: images.every((image) => image.naturalWidth > 0 && image.naturalHeight > 0),
    loadingCount: loading.length,
    geometry,
  };
}

export function isCaptureReady(snapshot, previousGeometry = snapshot.geometry) {
  return snapshot.imageCount > 0 && snapshot.imagesReady && snapshot.loadingCount === 0 && snapshot.geometry === previousGeometry;
}
