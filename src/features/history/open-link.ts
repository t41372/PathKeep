import { useCallback } from 'react'
import { toast } from 'sonner'
import { supportClient } from '@/lib/backend-client/support'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'

/**
 * Opens a page in the system browser, or a local file in the app that handles
 * it. The backend opens a file:// URL only if it is a page in the user's
 * history: such a URL can launch an application, and a link in an Ask answer
 * is written by a model. A refusal or failure is a toast, not a crash.
 */
export function useOpenPage() {
  const { t } = useI18n()
  return useCallback(
    async (url: string, failedMessage: string) => {
      try {
        await supportClient.openVisitedPage(url)
      } catch (error) {
        toast.error(failedMessage, {
          description:
            commandErrorCode(error) === 'not-in-history'
              ? t('common.notInHistory')
              : error instanceof Error
                ? error.message
                : undefined,
        })
      }
    },
    [t],
  )
}
