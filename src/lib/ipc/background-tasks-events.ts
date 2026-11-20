/**
 * The event the backend sends after a committed write changed the run
 * ledger's rows on the background tasks page: a browsing backup or import,
 * an Insights or search-index job, or work on the whole archive began,
 * moved on or ended, from this window or anywhere else (the menu bar, the
 * schedule). One event per transaction, so a busy queue sends many.
 *
 * Not responsible for: what to refetch (`src/lib/queries/background-tasks.ts`).
 */
import {
  BACKGROUND_TASKS_CHANGED_EVENT,
  type BackgroundTasksChangedEvent,
} from '../backend-client/background-tasks'
import { listenDesktopEvent } from './events'

export function subscribeToBackgroundTasksChanged(
  listener: (event: BackgroundTasksChangedEvent) => void,
) {
  return listenDesktopEvent<BackgroundTasksChangedEvent>(
    BACKGROUND_TASKS_CHANGED_EVENT,
    listener,
  )
}
