/**
 * Events the desktop shell sends that did not start in this window: the menu
 * bar icon's "Search history…", backups it ran, index jobs the background
 * queue finished, and the question asked when someone quits while work runs.
 * Through the dev bridge they arrive when the bridge runs beside the app
 * (`listenDesktopEvent`).
 */
import type { QuitRequest } from '../backend-client/desktop'
import type { BackupReport } from '../types'
import { listenDesktopEvent } from './events'

export interface BackupFinishedEvent {
  source: 'app' | 'menu-bar'
  report?: BackupReport | null
  error?: string | null
}

export function subscribeToOpenCommandPalette(listener: () => void) {
  return listenDesktopEvent<null>('pathkeep://open-command-palette', listener)
}

export function subscribeToBackupFinished(
  listener: (event: BackupFinishedEvent) => void,
) {
  return listenDesktopEvent<BackupFinishedEvent>(
    'pathkeep://backup-finished',
    listener,
  )
}

/**
 * An index job the background queue finished handling: it succeeded, failed,
 * was cancelled or paused, or waits for a retry. `state` is the job's state
 * after that.
 */
export interface IndexJobEndedEvent {
  jobId: number
  state: string
}

export function subscribeToIndexJobEnded(
  listener: (event: IndexJobEndedEvent) => void,
) {
  return listenDesktopEvent<IndexJobEndedEvent>(
    'pathkeep://ai-index-job-ended',
    listener,
  )
}

/**
 * The quit question as it changes: sent when a quit is held, when what is
 * running changes, and `null` once it is answered.
 */
export function subscribeToQuitRequest(
  listener: (request: QuitRequest | null) => void,
) {
  return listenDesktopEvent<QuitRequest | null>(
    'pathkeep://quit-request',
    listener,
  )
}
