import type { UpdateInstallState } from '../types'

export type UpdaterProgressListener = (event: UpdateInstallState) => void

export async function subscribeToUpdaterProgress(
  listener: UpdaterProgressListener,
) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<UpdateInstallState>(
      'pathkeep://updater-progress',
      ({ payload }) => {
        if (payload) listener(payload)
      },
    )
  } catch {
    return () => {}
  }
}
