/**
 * @file model-download.ts
 * @description Front-end subscription + in-flight latch for the in-app embedding model download.
 * @module lib/ipc/model-download
 *
 * ## Responsibilities
 * - Subscribe to the `pathkeep://model-download-progress` Tauri channel and forward each
 *   `ModelDownloadProgressEvent` (per-file started/finished + terminal done/error) to a listener.
 * - Hold a process-global "download in flight" latch so the download state survives a component
 *   remount or a snapshot poll that momentarily drops `staticEmbedding`, preventing a silent
 *   re-trigger of the (non-idempotent, bandwidth-heavy) download command.
 *
 * ## Not responsible for
 * - Starting or cancelling the download (those are `backend` commands).
 * - Rendering any UI or deciding copy.
 * - Aggregating bytes/percent: the backend stream is per-file with unknown sizes, so callers show
 *   honest indeterminate progress rather than a fabricated bar.
 *
 * ## Dependencies
 * - `@tauri-apps/api/event` for the live channel (absent under the dev HTTP bridge / browser
 *   preview, where the subscription degrades to a no-op unsubscribe by design).
 *
 * ## Why the latch mirrors the backend cancel flag
 * - The Rust side guards an in-flight download with a single process-global `AtomicBool`. This is
 *   the front-end counterpart: a single module-scoped boolean is the only state that reliably
 *   survives a React remount within one JS session, so the Download button cannot re-fire while a
 *   download is still running but the panel briefly unmounted.
 */

import type { ModelDownloadProgressEvent } from '../types'

/** Tauri event channel for in-app model download progress. Must match the Rust literal. */
export const MODEL_DOWNLOAD_PROGRESS_EVENT =
  'pathkeep://model-download-progress'

export type ModelDownloadProgressListener = (
  event: ModelDownloadProgressEvent,
) => void

/**
 * Subscribes to the in-app model download progress channel.
 *
 * Returns an unsubscribe function. When the Tauri event bridge is unavailable (browser preview /
 * dev HTTP bridge) it degrades to a no-op unsubscribe instead of throwing, so callers can always
 * subscribe unconditionally.
 */
export async function subscribeToModelDownloadProgress(
  listener: ModelDownloadProgressListener,
): Promise<() => void> {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<ModelDownloadProgressEvent>(
      MODEL_DOWNLOAD_PROGRESS_EVENT,
      ({ payload }) => {
        if (payload) listener(payload)
      },
    )
  } catch {
    return () => {}
  }
}

// Process-global download-in-flight latch (mirrors the backend's global cancel flag). A module
// scalar is intentional: it is the only state that survives a component remount within one JS
// session, which is exactly what guards the Download button from re-firing the command.
let downloadInFlight = false

/** Marks a download as started so a remount/poll cannot re-enable the Download button. */
export function markModelDownloadStarted(): void {
  downloadInFlight = true
}

/** Clears the in-flight latch on any terminal outcome (done / error / cancel). */
export function markModelDownloadSettled(): void {
  downloadInFlight = false
}

/** Whether a download is currently in flight (true between started and the terminal event). */
export function isModelDownloadInFlight(): boolean {
  return downloadInFlight
}
