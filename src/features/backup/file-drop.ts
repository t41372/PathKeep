/**
 * File picking and dropping for the desktop app. Tauri hands dropped files to
 * the window as paths (HTML5 drop events never fire for them), and the dialog
 * plugin returns paths too, which is what the backend needs. In a plain
 * browser neither exists, so callers fall back to typing a path.
 */
import { useEffect, useRef, useState } from 'react'
import { hasTauriGuestApi } from '@/lib/runtime'

export const canUseNativeFiles = hasTauriGuestApi

export async function pickTakeoutFile(title: string): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({
    title,
    multiple: false,
    directory: false,
    filters: [{ name: 'Google Takeout', extensions: ['zip'] }],
  })
  return typeof picked === 'string' ? picked : null
}

/** Reports the first dropped path anywhere on the window while `enabled`. */
export function useWindowFileDrop(
  onDrop: (path: string) => void,
  enabled: boolean,
) {
  const [hovering, setHovering] = useState(false)
  const handler = useRef(onDrop)
  handler.current = onDrop

  useEffect(() => {
    if (!enabled || !canUseNativeFiles()) return
    let cancelled = false
    let unlisten: (() => void) | undefined
    void import('@tauri-apps/api/webview')
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent(({ payload }) => {
          if (payload.type === 'enter') setHovering(true)
          else if (payload.type === 'leave') setHovering(false)
          else if (payload.type === 'drop') {
            setHovering(false)
            if (payload.paths[0]) handler.current(payload.paths[0])
          }
        }),
      )
      .then((off) => (cancelled ? off() : (unlisten = off)))
      .catch(() => undefined)
    return () => {
      cancelled = true
      setHovering(false)
      unlisten?.()
    }
  }, [enabled])

  return hovering
}
