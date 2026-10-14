/**
 * The strip a scrolling screen can be dragged by on macOS, where the window has
 * no title bar of its own (see drag-region.ts for the rest of the marks).
 */
import { hasMacOverlayTitlebar } from '@/lib/runtime'

/**
 * The first child of a screen's scrolling column. It takes no room (the screen
 * has at least as much top padding), so at rest it covers nothing.
 */
export function TitleBarStrip() {
  if (!hasMacOverlayTitlebar()) return null
  return (
    <div
      data-tauri-drag-region
      aria-hidden
      className="sticky top-0 z-10 -mb-7 h-7 shrink-0"
    />
  )
}
