/**
 * A folder to look at again on the Agent backup page: `?renew={rootId}`.
 * A folder made again at the path that was approved is a different folder,
 * and a backup reads it only once it is approved again; a retry that needs
 * that, or the folder's own row, opens this look.
 */
import { useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'
import { spaceBackupPath } from '@/lib/spaces'

export const RENEW_PARAM = 'renew'

/** The backup page's address that opens the look at this folder. */
export function renewPath(rootId: string) {
  return spaceBackupPath(
    'agent',
    `?${RENEW_PARAM}=${encodeURIComponent(rootId)}`,
  )
}

export function useRenewParam() {
  const [params, setParams] = useSearchParams()
  const rootId = params.get(RENEW_PARAM)
  const clear = useCallback(
    () =>
      setParams(
        (current) => {
          const next = new URLSearchParams(current)
          next.delete(RENEW_PARAM)
          return next
        },
        { replace: true },
      ),
    [setParams],
  )
  return { rootId, clear }
}
