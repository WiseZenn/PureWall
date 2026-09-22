export interface ThumbnailRetry {
  attempt: number;
  delayMs: number;
}

const MAX_AUTOMATIC_RETRIES = 2;
const INITIAL_RETRY_DELAY_MS = 250;

export function nextThumbnailRetry(attemptsSoFar: number): ThumbnailRetry | null {
  if (attemptsSoFar >= MAX_AUTOMATIC_RETRIES) return null;

  const attempt = Math.max(0, attemptsSoFar) + 1;
  return {
    attempt,
    delayMs: INITIAL_RETRY_DELAY_MS * 2 ** (attempt - 1),
  };
}