export interface ThumbnailRequestScheduler {
  schedule(paths: string[]): void;
  cancel(): void;
}

export function createThumbnailRequestScheduler(
  request: (paths: string[]) => void | Promise<void>,
  delayMs: number,
): ThumbnailRequestScheduler {
  let timer: ReturnType<typeof setTimeout> | undefined;

  function cancel() {
    if (timer !== undefined) {
      globalThis.clearTimeout(timer);
      timer = undefined;
    }
  }

  function schedule(paths: string[]) {
    const visiblePaths = Array.from(new Set(paths.filter(Boolean)));
    cancel();
    if (visiblePaths.length === 0) return;

    timer = globalThis.setTimeout(() => {
      timer = undefined;
      void request(visiblePaths);
    }, delayMs);
  }

  return { schedule, cancel };
}
