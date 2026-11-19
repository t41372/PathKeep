/** "Show in Finder" (or the folder on other systems) for paths Settings writes. */
import { toast } from 'sonner'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { isMacOsHost } from '@/lib/runtime'

export function revealLabelKey() {
  return isMacOsHost()
    ? ('settingsStorage.location.showFinder' as const)
    : ('settingsStorage.location.show' as const)
}

export function reveal(path: string, failedTitle: string) {
  return supportClient.openPathInFileManager(path).catch((error) =>
    toast.error(failedTitle, {
      description: describeError(error, 'open_path_in_file_manager'),
    }),
  )
}

/** The folder that holds `path`: everything before its last separator. */
function parentOf(path: string) {
  const cut = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return cut > 0 ? path.slice(0, cut) : null
}

/**
 * Shows a file a refusal named. The file can be gone (a deleted conversation
 * file, a destination never written), so when it can't be shown its folder
 * is shown instead.
 */
export function revealFile(path: string, failedTitle: string) {
  return supportClient.openPathInFileManager(path).catch((error: unknown) => {
    const parent = parentOf(path)
    if (parent) return reveal(parent, failedTitle)
    toast.error(failedTitle, {
      description: describeError(error, 'open_path_in_file_manager'),
    })
  })
}
