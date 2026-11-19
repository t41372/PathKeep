/**
 * The events a conversation backup sends while it runs and when it ends,
 * whichever window, button or schedule started it: its progress, its task's
 * state, its end, and "what the backup page reads has changed".
 *
 * Not responsible for: what to refetch or patch (`src/lib/queries/agent-backup.ts`).
 */
import {
  AGENT_CATALOG_CHANGED_EVENT,
  AGENT_SETTINGS_CHANGED_EVENT,
  BACKGROUND_TASK_CHANGED_EVENT,
  KIND_BACKUP_FINISHED_EVENT,
  KIND_BACKUP_PROGRESS_EVENT,
  type BackgroundTaskChangedEvent,
  type KindBackupFinishedEvent,
  type KindBackupProgressEvent,
} from '../backend-client/agent-backup'
import { listenDesktopEvent } from './events'

export interface AgentBackupListeners {
  progress: (event: KindBackupProgressEvent) => void
  taskChanged: (event: BackgroundTaskChangedEvent) => void
  finished: (event: KindBackupFinishedEvent) => void
  /** The overview, sources or runs changed. */
  catalogChanged: () => void
  /** The tools, folders or `.git` choice changed. */
  settingsChanged: () => void
}

/** Listens to all five until the returned function is called. */
export async function subscribeToAgentBackup(
  listeners: AgentBackupListeners,
): Promise<() => void> {
  const stops = await Promise.all([
    listenDesktopEvent(KIND_BACKUP_PROGRESS_EVENT, listeners.progress),
    listenDesktopEvent(BACKGROUND_TASK_CHANGED_EVENT, listeners.taskChanged),
    listenDesktopEvent(KIND_BACKUP_FINISHED_EVENT, listeners.finished),
    listenDesktopEvent(AGENT_CATALOG_CHANGED_EVENT, listeners.catalogChanged),
    listenDesktopEvent(AGENT_SETTINGS_CHANGED_EVENT, listeners.settingsChanged),
  ])
  return () => {
    for (const stop of stops) stop()
  }
}
