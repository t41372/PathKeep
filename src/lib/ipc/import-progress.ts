/**
 * Import-progress event subscription helper.
 *
 * Why this file exists:
 * - Import is a foreground workflow that can still run long enough for the UI
 *   to look frozen unless we surface incremental progress updates.
 * - The helper keeps the event wiring small and testable instead of pushing the
 *   Tauri API detail into route components.
 */

import type { ImportProgressEvent } from '../types'

export type ImportProgressListener = (event: ImportProgressEvent) => void

export async function subscribeToImportProgress(
  listener: ImportProgressListener,
) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<ImportProgressEvent>(
      'pathkeep://import-progress',
      ({ payload }) => {
        if (payload) listener(payload)
      },
    )
  } catch {
    return () => {}
  }
}
