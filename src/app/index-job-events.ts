/**
 * Reads the app's state again when the background queue finishes an index
 * job, whichever screen is open: a build or an after-backup update can end
 * after the user left Settings → AI, and History's semantic search, Settings
 * and Background work all read that state. Not responsible for the queue or
 * the jobs themselves.
 */
import { useEffect } from 'react'
import { aiQueueKey } from '@/features/settings/background/use-background-work'
import { appClient } from '@/lib/backend-client/app'
import { subscribeToIndexJobEnded } from '@/lib/ipc/desktop-events'
import { queryClient, queryKeys } from '@/lib/query'

async function refreshAfterIndexJob() {
  queryClient.setQueryData(queryKeys.snapshot, await appClient.getSnapshot())
  await queryClient.invalidateQueries({ queryKey: aiQueueKey })
}

/** Listens while `active`: the state can only be read with the archive open. */
export function useIndexJobEvents(active: boolean) {
  useEffect(() => {
    if (!active) return
    const unsubscribe = subscribeToIndexJobEnded(() => {
      void refreshAfterIndexJob().catch(() => undefined)
    })
    return () => void unsubscribe.then((stop) => stop())
  }, [active])
}
