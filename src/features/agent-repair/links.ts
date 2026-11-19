/**
 * The repair dialog's address: `?repair=agent` on whatever screen is open,
 * so the dialog opens over it (from the damage notice, or from a move or
 * encryption dialog that was refused) and closing it leaves the person there.
 *
 * Not responsible for: the dialog (`agent-repair-dialog.tsx`).
 */
import { useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'

export const REPAIR_PARAM = 'repair'
export const REPAIR_AGENT = 'agent'

/** Opens the dialog over the current screen. */
export function useOpenAgentRepair() {
  const [, setParams] = useSearchParams()
  return useCallback(
    () =>
      setParams((current) => {
        const next = new URLSearchParams(current)
        next.set(REPAIR_PARAM, REPAIR_AGENT)
        return next
      }),
    [setParams],
  )
}

/** Whether the dialog is open, and closing it. */
export function useAgentRepairParam() {
  const [params, setParams] = useSearchParams()
  const open = params.get(REPAIR_PARAM) === REPAIR_AGENT
  const close = useCallback(
    () =>
      setParams(
        (current) => {
          const next = new URLSearchParams(current)
          next.delete(REPAIR_PARAM)
          return next
        },
        { replace: true },
      ),
    [setParams],
  )
  return { open, close }
}
