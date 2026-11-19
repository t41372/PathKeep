/**
 * Turning local semantic search on with the built-in model: download the
 * model, then ask for the index build that download allows.
 *
 * Responsibilities:
 * - `runLocalSemanticSetup`, the one entry point Settings → AI and onboarding
 *   use: the model must be on disk before the build, because the build loads it.
 * - The build request names the built-in model and the finished transfer. The
 *   backend refuses it when the saved settings no longer want the built-in
 *   model (a switch to another service never builds, and never sends page text
 *   anywhere), and answers a repeat for the same transfer, from any window,
 *   with the first job.
 * - Telling a stop the user asked for from a failure: a cancelled download, or
 *   a start or build the backend refused because the saved settings no longer
 *   want the model, is `ModelDownloadCancelled`.
 *
 * Not responsible for:
 * - The download itself and what the screens show of it (`model-download.ts`).
 * - Saving the settings, or showing errors (the caller decides).
 */

import { appClient } from '../backend-client/app'
import { intelligenceClient } from '../backend-client/intelligence'
import {
  BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  type AiSettings,
} from '../types'
import { downloadModel } from './model-download'

/**
 * The setup stopped because the user asked for it: Cancel, or a saved setting
 * (semantic search or AI off, another service chosen). Never shown as a failure.
 */
export class ModelDownloadCancelled extends Error {
  constructor() {
    super('model download cancelled')
    this.name = 'ModelDownloadCancelled'
  }
}

/**
 * Nothing was heard from the download for the idle window, so the setup gave
 * up. The screen says so in its own words; the message is for logs.
 */
export class ModelDownloadStalled extends Error {
  constructor() {
    super('the model download sent nothing within the idle window')
    this.name = 'ModelDownloadStalled'
  }
}

/** Whether these settings want the built-in model: AI and semantic search on, and it chosen. */
export function wantsBuiltInModel(ai: AiSettings) {
  return (
    ai.enabled &&
    ai.semanticIndexEnabled &&
    (ai.embeddingProviderId ?? BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID) ===
      BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
  )
}

/**
 * Read from the backend, not this window's cache: a refusal follows a save that
 * may not have reached this window yet, or was made in another one.
 */
async function savedSettingsWantIt() {
  return wantsBuiltInModel((await appClient.getSnapshot()).config.ai)
}

/**
 * Downloads (or verifies) the built-in model, then asks for an index build of
 * every visit the index lacks. It never wipes an index already there: turning
 * semantic search off and on again continues it.
 *
 * Throws `ModelDownloadCancelled` for a stop the user asked for,
 * `ModelDownloadStalled` when the download went silent, and the error itself
 * otherwise. A failed download is also shown by `useModelDownloadProgress`.
 */
export async function runLocalSemanticSetup(): Promise<void> {
  const outcome = await downloadModel(savedSettingsWantIt)
  switch (outcome.kind) {
    case 'cancelled':
      throw new ModelDownloadCancelled()
    case 'silent':
      throw new ModelDownloadStalled()
    case 'failed':
      throw new Error(outcome.message)
    case 'refused':
      throw outcome.error
    case 'done':
      break
  }
  try {
    // `incremental`: every visit the index lacks. `full` would wipe it first.
    await intelligenceClient.buildIndex({
      providerId: BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
      transferId: outcome.transferId,
      fullRebuild: false,
      clearOnly: false,
      scope: 'incremental',
    })
  } catch (error) {
    if (!(await savedSettingsWantIt().catch(() => true)))
      throw new ModelDownloadCancelled()
    throw error
  }
}
