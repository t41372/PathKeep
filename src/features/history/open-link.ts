import { toast } from 'sonner'
import { supportClient } from '@/lib/backend-client/support'

/** Opens a page in the system browser; a failure is a toast, not a crash. */
export async function openInBrowser(url: string, failedMessage: string) {
  try {
    await supportClient.openExternalUrl(url)
  } catch (error) {
    toast.error(failedMessage, {
      description: error instanceof Error ? error.message : undefined,
    })
  }
}
