/**
 * Events the desktop shell sends that did not start in this window: the menu
 * bar icon's "Search history…" and backups it ran. Outside Tauri (the browser
 * bridge) there is no menu bar, so subscribing does nothing.
 */
import type { BackupReport } from '../types'

export interface BackupFinishedEvent {
  source: 'app' | 'menu-bar'
  report?: BackupReport | null
  error?: string | null
}

async function subscribe<T>(event: string, listener: (payload: T) => void) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<T>(event, ({ payload }) => listener(payload))
  } catch {
    return () => {}
  }
}

export function subscribeToOpenCommandPalette(listener: () => void) {
  return subscribe<null>('pathkeep://open-command-palette', listener)
}

export function subscribeToBackupFinished(
  listener: (event: BackupFinishedEvent) => void,
) {
  return subscribe<BackupFinishedEvent>('pathkeep://backup-finished', listener)
}
