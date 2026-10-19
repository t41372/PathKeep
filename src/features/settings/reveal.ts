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
