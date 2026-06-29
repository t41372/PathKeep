/**
 * @file model-download.ts
 * @description Front-end subscription + in-flight latch + progress hook for the in-app embedding model download.
 * @module lib/ipc/model-download
 *
 * ## Responsibilities
 * - Subscribe to the `pathkeep://model-download-progress` Tauri channel and forward each
 *   `ModelDownloadProgressEvent` (per-file started/progress/finished + terminal done/error) to a listener.
 * - Hold a process-global "download in flight" latch so the download state survives a component
 *   remount or a snapshot poll that momentarily drops `staticEmbedding`, preventing a silent
 *   re-trigger of the (non-idempotent, bandwidth-heavy) download command.
 * - Expose `useModelDownloadProgress` — a React hook that drives the full phase/bytes state machine
 *   for any component rendering download UI (the Settings base-tier panel, the Jobs page progress
 *   view, etc.). Block C (jobs page) imports this hook read-only.
 *
 * ## Not responsible for
 * - Starting or cancelling the download (those are `backend` commands).
 * - Rendering any UI or deciding copy.
 *
 * ## Dependencies
 * - `@tauri-apps/api/event` for the live channel (absent under the dev HTTP bridge / browser
 *   preview, where the subscription degrades to a no-op unsubscribe by design).
 * - `react` for the hook exports (`useState`, `useEffect`, `useRef`).
 *
 * ## Why the latch mirrors the backend cancel flag
 * - The Rust side guards an in-flight download with a single process-global `AtomicBool`. This is
 *   the front-end counterpart: a single module-scoped boolean is the only state that reliably
 *   survives a React remount within one JS session, so the Download button cannot re-fire while a
 *   download is still running but the panel briefly unmounted.
 */

import { useEffect, useRef, useState } from 'react'
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

// ─── useModelDownloadProgress ─────────────────────────────────────────────────

/**
 * Stable progress shape returned by `useModelDownloadProgress`.
 *
 * - `phase` 'idle'       — no download has started; the Download button is shown.
 * - `phase` 'downloading' — download in flight; the progress bar + cancel are shown.
 * - `phase` 'ready'      — weights are present and verified. May come from `modelDownloaded`
 *                          prop OR from a live `done` event (fixes the stuck-on-downloading
 *                          bug that existed in the previous per-component implementation).
 * - `phase` 'failed'     — terminal error that was NOT caused by a user cancel; retry is shown.
 *
 * `downloadedBytes` / `totalBytes` are honest running totals accumulated across all files.
 * When `totalBytes` is 0 the server did not supply Content-Length; the UI should show an
 * indeterminate shimmer instead of a fabricated percentage.
 */
export interface ModelDownloadProgress {
  phase: 'idle' | 'downloading' | 'ready' | 'failed'
  downloadedBytes: number
  totalBytes: number
  currentFile: string | null
  error: string | null
}

/**
 * React hook that subscribes to in-app embedding model download progress and drives a clean
 * phase / byte state object for any component that renders download UI.
 *
 * ## Key behaviors
 * - `done` → phase immediately becomes **'ready'** without waiting for a snapshot re-poll.
 *   This fixes the bug in the previous per-component handler where `done` kept the spinner
 *   until `staticEmbedding.modelDownloaded` was re-polled from the backend.
 * - `modelDownloaded: true` overrides the phase to 'ready' regardless of subscription events.
 * - `cancelledRef.current = true` (set **before** the cancel IPC fires) maps the subsequent
 *   terminal `error` event to 'idle' instead of 'failed', so a user cancel does not look like
 *   a network failure.
 * - Byte accumulation: `downloadedBytes` = Σ finished-file totals + current-file downloaded;
 *   `totalBytes` = Σ known per-file totals (updated from both `fileStarted.totalBytes` and
 *   `fileProgress.totalBytes`). When unknown (all zeros), callers render an indeterminate bar.
 * - No listener leak: subscription is cleaned up on unmount and on the unmount-before-resolve
 *   race path.
 * - Latch: initialized from `isModelDownloadInFlight()` so a remount mid-download starts in
 *   'downloading' phase immediately, before the next event arrives.
 *
 * @param modelDownloaded - Authoritative "weights are already present" flag from the backend
 *   snapshot. When true, phase is 'ready' regardless of in-progress subscription state.
 * @param cancelledRef - Optional caller-owned mutable ref. Set `.current = true` before calling
 *   the cancel IPC command so the resulting terminal `error` maps to 'idle' rather than 'failed'.
 *   Use `useRef<boolean>(false)` at the call site.
 *
 * @returns Stable `ModelDownloadProgress` object. Block C (jobs page) imports this read-only.
 */
export function useModelDownloadProgress(
  modelDownloaded: boolean,
  cancelledRef?: { current: boolean },
  restartNonce = 0,
): ModelDownloadProgress {
  const [phase, setPhase] = useState<ModelDownloadProgress['phase']>(() =>
    modelDownloaded ? 'ready' : downloadInFlight ? 'downloading' : 'idle',
  )
  const [downloadedBytes, setDownloadedBytes] = useState(0)
  const [totalBytes, setTotalBytes] = useState(0)
  const [currentFile, setCurrentFile] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Per-file byte accumulation — all refs so intermediate writes don't cause extra renders.
  const finishedBytesRef = useRef(0)
  // Maps file path → known total bytes for that file (updated by both fileStarted and fileProgress).
  const fileTotalMapRef = useRef(new Map<string, number>())
  const currentFileBytesRef = useRef(0)

  useEffect(() => {
    if (modelDownloaded) {
      setPhase('ready')
      return
    }

    let active = true
    let unsub = () => {}

    void subscribeToModelDownloadProgress((event) => {
      if (!active) return
      switch (event.kind) {
        case 'fileStarted': {
          markModelDownloadStarted()
          fileTotalMapRef.current.set(event.file, event.totalBytes)
          currentFileBytesRef.current = 0
          const knownTotal = sumMapValues(fileTotalMapRef.current)
          /* v8 ignore next -- split('/') always yields ≥1 element, so pop() is never nullish; the ?? only guards a structurally-impossible malformed path. */
          const basename = event.file.split('/').pop() ?? event.file
          setPhase('downloading')
          setCurrentFile(basename)
          setTotalBytes(knownTotal)
          setDownloadedBytes(finishedBytesRef.current)
          // Clear any prior error so a retry that reaches the network never shows a stale failure
          // (a consumer that renders `error` — Block C — must not surface a healed failure).
          setError(null)
          break
        }
        case 'fileProgress': {
          // fileProgress also carries totalBytes — may be more accurate than fileStarted.
          if (event.totalBytes > 0) {
            fileTotalMapRef.current.set(event.file, event.totalBytes)
          }
          currentFileBytesRef.current = event.downloadedBytes
          const knownTotal = sumMapValues(fileTotalMapRef.current)
          setDownloadedBytes(finishedBytesRef.current + event.downloadedBytes)
          setTotalBytes(knownTotal)
          break
        }
        case 'fileFinished': {
          // Accumulate this file's bytes into the finished total.
          const fileTotal =
            fileTotalMapRef.current.get(event.file) ??
            currentFileBytesRef.current
          finishedBytesRef.current += fileTotal
          currentFileBytesRef.current = 0
          setDownloadedBytes(finishedBytesRef.current)
          setCurrentFile(null)
          break
        }
        case 'done': {
          // KEY FIX: set 'ready' IMMEDIATELY — do NOT wait for a snapshot re-poll.
          // The old per-component handler left the spinner active after `done`, causing the
          // "stuck on downloading" bug visible until the next poll confirmed modelDownloaded.
          markModelDownloadSettled()
          setPhase('ready')
          setCurrentFile(null)
          setError(null)
          break
        }
        case 'error': {
          markModelDownloadSettled()
          setCurrentFile(null)
          const wasCancelled = cancelledRef?.current === true
          setPhase(wasCancelled ? 'idle' : 'failed')
          if (!wasCancelled) setError(event.message)
          break
        }
      }
    }).then((fn) => {
      if (active) unsub = fn
      else fn()
    })

    return () => {
      active = false
      unsub()
    }
    // cancelledRef is a stable ref object; its .current is read inside the effect at event time.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [modelDownloaded])

  // A user-initiated (re)start clears any prior terminal state IMMEDIATELY so the panel never shows a
  // stale "Download failed" during the connection-setup window before the first `fileStarted` arrives
  // (the same "stuck" lie this hook set out to kill, on the retry path). `restartNonce` is bumped by
  // the caller on each Download/Retry click; nonce 0 is the initial mount, where the computed phase
  // (from `modelDownloaded`/the latch) must stand. The subscription stays put — only state resets.
  useEffect(() => {
    if (restartNonce === 0) return
    finishedBytesRef.current = 0
    fileTotalMapRef.current.clear()
    currentFileBytesRef.current = 0
    setDownloadedBytes(0)
    setTotalBytes(0)
    setCurrentFile(null)
    setError(null)
    setPhase('downloading')
  }, [restartNonce])

  // Authoritative override: if the snapshot says the model is present, always return 'ready'.
  // This bridges the gap between a `done` event and the next snapshot poll so there is no flash.
  const effectivePhase: ModelDownloadProgress['phase'] = modelDownloaded
    ? 'ready'
    : phase

  return {
    phase: effectivePhase,
    downloadedBytes,
    totalBytes,
    currentFile,
    error,
  }
}

/** Sum all values in a Map<string, number>. */
function sumMapValues(map: Map<string, number>): number {
  let total = 0
  for (const v of map.values()) total += v
  return total
}
