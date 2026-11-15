/**
 * Dragging the window by its background on Windows and Linux, where the app
 * draws the window's frame (`drawsWindowControls`). A press on a marked spot
 * (`WINDOW_DRAG_MARK`, from `drag-region.ts`) only gets ready: the window
 * starts to follow once the pointer has moved a few pixels with the button
 * still down. A double click maximizes or restores the window, as on a title
 * bar.
 *
 * Tauri's own mark (`data-tauri-drag-region`) asks the window manager to move
 * the window on the press itself. A press released at once, as a trackpad tap
 * is, can reach the window manager after the release, and Xfce's then moves
 * the window with the pointer until the next click (BACKLOG
 * `WORK-DRAG-ON-MOVE`). Asking only once the pointer moves, the button is
 * still down when the window manager takes over, and a tap moves nothing.
 * macOS keeps Tauri's mark: it has the system's title bar behaviour around
 * the traffic lights, the tap was only ever seen with Xfce, and the macOS
 * window cannot be checked by hand here (`WORK-NATIVE-WINDOW-CHECK`).
 *
 * Responsible for: these listeners. Not responsible for: which spots are
 * marked (`drag-region.ts`) or the window buttons (`window-controls.tsx`).
 */
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect } from 'react'
import { WINDOW_DRAG_MARK } from '@/components/app/drag-region'
import { drawsWindowControls, hasTauriGuestApi } from '@/lib/runtime'

/**
 * How far the pointer moves with the button down before the window follows,
 * in CSS pixels: Windows' own drag threshold (`SM_CXDRAG`). A tap's jitter
 * stays under it.
 */
const DRAG_AFTER = 4

/** Mounted once at the app root, beside the window buttons. */
export function WindowDrag() {
  useEffect(() => {
    if (!drawsWindowControls() || !hasTauriGuestApi()) return
    return listen(document)
  }, [])
  return null
}

/** Only the marked element itself drags, not what sits on it. */
const marked = (event: MouseEvent) =>
  event.target instanceof HTMLElement &&
  event.target.hasAttribute(WINDOW_DRAG_MARK)

function listen(target: Document) {
  let pressedAt: { x: number; y: number } | null = null
  const down = (event: MouseEvent) => {
    pressedAt = null
    if (event.button !== 0 || !marked(event)) return
    // No text selection starts from the background.
    event.preventDefault()
    if (event.detail === 2) {
      void getCurrentWindow().toggleMaximize()
      return
    }
    if (event.detail === 1) pressedAt = { x: event.screenX, y: event.screenY }
  }
  const move = (event: MouseEvent) => {
    if (!pressedAt) return
    if ((event.buttons & 1) === 0) {
      pressedAt = null
      return
    }
    const moved = Math.hypot(
      event.screenX - pressedAt.x,
      event.screenY - pressedAt.y,
    )
    if (moved < DRAG_AFTER) return
    pressedAt = null
    void getCurrentWindow().startDragging()
  }
  const up = () => {
    pressedAt = null
  }
  target.addEventListener('mousedown', down)
  target.addEventListener('mousemove', move)
  target.addEventListener('mouseup', up)
  return () => {
    target.removeEventListener('mousedown', down)
    target.removeEventListener('mousemove', move)
    target.removeEventListener('mouseup', up)
  }
}
