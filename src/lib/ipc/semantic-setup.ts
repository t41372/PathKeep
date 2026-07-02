/**
 * @file semantic-setup.ts
 * @description Orchestrates the real, background local-semantic-search opt-in: ensure the on-device
 *              embedding model is present, then enqueue a full index build.
 * @module lib/ipc/semantic-setup
 *
 * ## Responsibilities
 * - Provide `runLocalSemanticSetup`, the single background entry point the onboarding "Enable" opt-in
 *   fires (via the shell-data action) once the archive is initialized and the first backup ran.
 * - Guarantee the MANDATORY ordering: the static embedding model must be present on disk BEFORE the
 *   index build runs, because the embedding job loads the model via `load_default`, which bails if the
 *   weights are missing. Firing both concurrently would make the build job fail.
 * - Drive the model-download phase off the existing `pathkeep://model-download-progress` channel and
 *   the process-global in-flight latch, so it composes with the Settings/Activity download UI instead
 *   of re-implementing a second download owner.
 * - Guard against a stranded latch with an IDLE watchdog: if the progress channel goes completely
 *   silent (a degraded event bridge that delivers no events, or a dead download thread), reject so the
 *   subscription + the process-global in-flight flag are always released. It is reset by EVERY progress
 *   event, so a legitimately slow-but-active download is never aborted (it is NOT a total-duration cap).
 *
 * ## Not responsible for
 * - Rendering any UI, deciding copy, or surfacing errors — it lets the caller (the shell action)
 *   decide, and the shell action swallows so onboarding never surfaces a blocking failure.
 * - Persisting `config.ai` (the onboarding route owner does that) or kicking the ambient task bar
 *   (the shell action refreshes runtime status; the index build surfaces via the runtime poller).
 * - Cancelling a download or managing the queue drain (those are `backend` commands + the queue).
 *
 * ## Dependencies
 * - `../backend-client` for the two background commands (`downloadStaticEmbeddingModel`, `buildAiIndex`).
 * - `./model-download` for the progress subscription + the shared in-flight latch.
 */

import { backend } from '../backend-client'
import {
  markModelDownloadSettled,
  markModelDownloadStarted,
  subscribeToModelDownloadProgress,
} from './model-download'

/**
 * Idle window for the model-download progress channel. If NO event of any kind arrives for this long,
 * the channel is treated as dead (degraded event bridge / dead download thread) and the setup rejects
 * so the subscription + the process-global in-flight latch are released instead of hanging forever.
 *
 * This is an IDLE watchdog, not a total-duration cap: every progress event resets it, so a slow but
 * ACTIVE large-model download is never aborted — only a truly silent channel trips it. Kept generous
 * so a stalled-but-recovering connection between throttled progress ticks is not misread as dead.
 */
export const MODEL_DOWNLOAD_IDLE_TIMEOUT_MS = 120_000

/**
 * Enables local semantic search in the background: ensures the on-device static embedding model is
 * downloaded/verified, THEN enqueues a full index build (which surfaces in the ambient task bar).
 *
 * Fire-and-forget friendly: the caller invokes it with `void`; it never blocks the UI. Errors are NOT
 * swallowed here — they propagate so the caller can decide (the onboarding shell action swallows, and
 * the download phase already surfaces on the Activity page for retry from Settings → AI).
 */
export async function runLocalSemanticSetup(): Promise<void> {
  await ensureStaticModelDownloaded()
  await backend.buildAiIndex({
    fullRebuild: true,
    clearOnly: false,
    scope: 'full',
  })
}

/**
 * Resolves once the static embedding model's weights are present + verified (a terminal `done` on the
 * progress channel), and rejects on a terminal `error`, a command-level rejection, or the idle
 * watchdog tripping. Safe to call unconditionally: when the model is already present the backend
 * verifies and emits `done` fast (no network).
 *
 * `markModelDownloadSettled()` runs on EVERY exit path (each routes through the single-settle latch),
 * so the process-global in-flight flag can never be stranded — including the watchdog-reject path.
 */
async function ensureStaticModelDownloaded(): Promise<void> {
  let resolveDownload!: () => void
  let rejectDownload!: (error: Error) => void
  const downloadSettled = new Promise<void>((resolve, reject) => {
    resolveDownload = resolve
    rejectDownload = reject
  })

  // Idle watchdog: armed when the download starts, reset on EVERY channel event, and cleared on settle
  // and in the finally so it never fires late or leaks a timer.
  let watchdog: ReturnType<typeof setTimeout> | null = null
  const clearWatchdog = () => {
    if (watchdog !== null) {
      clearTimeout(watchdog)
      watchdog = null
    }
  }

  // Single terminal latch: the first of {done, error, command-rejection, idle-watchdog} wins, clears
  // the watchdog + the process-global in-flight flag exactly once, so a late duplicate event (or a
  // watchdog firing just after a real terminal) can never double-settle.
  let settled = false
  const settle = (action: () => void) => {
    if (settled) return
    settled = true
    clearWatchdog()
    markModelDownloadSettled()
    action()
  }

  const armWatchdog = () => {
    clearWatchdog()
    watchdog = setTimeout(() => {
      settle(() =>
        rejectDownload(
          new Error(
            'static embedding model download went silent (no progress within the idle window)',
          ),
        ),
      )
    }, MODEL_DOWNLOAD_IDLE_TIMEOUT_MS)
  }

  // Subscribe FIRST (awaited) so no progress/terminal event is missed before the download starts.
  const unsubscribe = await subscribeToModelDownloadProgress((event) => {
    // ANY event proves the channel is alive — reset the idle watchdog before handling terminals.
    armWatchdog()
    if (event.kind === 'done') {
      settle(resolveDownload)
    } else if (event.kind === 'error') {
      const { message } = event
      settle(() => rejectDownload(new Error(message)))
    }
  })

  // Race: a terminal event can fire synchronously while the subscription is still registering, so by
  // the time the unsubscribe fn arrives we may already be settled. Skip starting a download the
  // terminal already ended and release the listener immediately.
  if (!settled) {
    markModelDownloadStarted()
    // Arm the idle watchdog now so a degraded event bridge (no events ever arrive) or a dead download
    // thread cannot strand the promise + the process-global latch forever.
    armWatchdog()
    // The command spawns a background thread and returns immediately; the real terminal outcome still
    // arrives on the channel. A command-level rejection is itself terminal, so route it through the
    // same single-settle latch.
    backend.downloadStaticEmbeddingModel().catch((error: unknown) => {
      settle(() =>
        rejectDownload(
          error instanceof Error ? error : new Error(String(error)),
        ),
      )
    })
  }

  try {
    await downloadSettled
  } finally {
    clearWatchdog()
    unsubscribe()
  }
}
