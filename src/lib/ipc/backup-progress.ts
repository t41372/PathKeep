import type { BackupProgressEvent } from '../types'

export type BackupProgressListener = (event: BackupProgressEvent) => void

export async function subscribeToBackupProgress(
  listener: BackupProgressListener,
) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<BackupProgressEvent>(
      'pathkeep://backup-progress',
      ({ payload }) => {
        if (payload) listener(payload)
      },
    )
  } catch {
    return () => {}
  }
}
