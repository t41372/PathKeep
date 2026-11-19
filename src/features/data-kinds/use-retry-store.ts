/**
 * "Try again" for a kind whose storage will not open: asks the backend to
 * open Agent conversations' store again (`retry_agent_store`) and shows the
 * list it answers with, which says whether it opens now and, if not, why.
 * Browsing history is never touched by it.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  dataKindsClient,
  dataKindsErrorCode,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'

export function useRetryStore() {
  const { t } = useI18n()
  const [busy, setBusy] = useState(false)
  const retry = async () => {
    setBusy(true)
    try {
      const list = await dataKindsClient.retryAgentStore()
      queryClient.setQueryData(queryKeys.dataKinds, list)
    } catch (error) {
      // A store that still will not open comes back in the list, not here.
      if (dataKindsErrorCode(error) !== 'lock-required')
        toast.error(t('dataKinds.damage.retryFailed'), {
          description: describeError(error, 'retry_agent_store'),
        })
    } finally {
      await queryClient.invalidateQueries({ queryKey: queryKeys.dataKinds })
      setBusy(false)
    }
  }
  return { busy, retry }
}
