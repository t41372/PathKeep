/**
 * The run whose details are open on the Agent backup page: `?run={runRef}`,
 * so the header, the runs list, a notice and a link from elsewhere all open
 * the same sheet, also for a run that is not in the list (another kind's,
 * or one long ago).
 */
import { useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'

export const RUN_PARAM = 'run'

export function useRunParam() {
  const [params, setParams] = useSearchParams()
  const runRef = params.get(RUN_PARAM)
  const open = useCallback(
    (ref: string) =>
      setParams((current) => {
        const next = new URLSearchParams(current)
        next.set(RUN_PARAM, ref)
        return next
      }),
    [setParams],
  )
  const close = useCallback(
    () =>
      setParams(
        (current) => {
          const next = new URLSearchParams(current)
          next.delete(RUN_PARAM)
          return next
        },
        { replace: true },
      ),
    [setParams],
  )
  return { runRef, open, close }
}
