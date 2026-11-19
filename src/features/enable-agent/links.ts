/**
 * The enable dialog's address: `?enable=agent` on whatever screen is open,
 * so the dialog opens over it and closing it leaves the person there.
 *
 * Not responsible for: the dialog (`enable-agent-dialog.tsx`).
 */
import { useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'

export const ENABLE_PARAM = 'enable'
export const ENABLE_AGENT = 'agent'

/** Opens the dialog over the current screen. */
export function useOpenEnableAgent() {
  const [, setParams] = useSearchParams()
  return useCallback(
    () =>
      setParams((current) => {
        const next = new URLSearchParams(current)
        next.set(ENABLE_PARAM, ENABLE_AGENT)
        return next
      }),
    [setParams],
  )
}

/** Whether the dialog is open, and closing it. */
export function useEnableAgentParam() {
  const [params, setParams] = useSearchParams()
  const open = params.get(ENABLE_PARAM) === ENABLE_AGENT
  const close = useCallback(
    () =>
      setParams(
        (current) => {
          const next = new URLSearchParams(current)
          next.delete(ENABLE_PARAM)
          return next
        },
        { replace: true },
      ),
    [setParams],
  )
  return { open, close }
}
