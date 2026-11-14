/**
 * Where the window can be dragged from, as from any title bar. A desktop
 * window has no title bar of the system's on any platform (`windowFrame`), so
 * PathKeep marks its own:
 * - The window background, on every platform: the margin around the main
 *   panel (on Windows and Linux, the strip beside the window buttons), and
 *   the backdrop of the lock, start and error screens.
 * - On macOS also inside the panel, whose top lies in the title strip the
 *   traffic lights float over: a strip across the top of every scrolling
 *   screen (`title-bar.tsx`), which stays put while the content scrolls under
 *   it, and the empty parts of headers, toolbars and side lists. On Windows
 *   and Linux the panel starts below the strip, and inside it is content.
 *
 * Buttons, fields and links inside a marked area still work: Tauri does not
 * start a drag from them. A double click on a mark maximizes or restores the
 * window. In a browser nothing is marked.
 *
 * Responsible for: those marks. Not responsible for: the window or its
 * buttons.
 */
import { hasMacOverlayTitlebar, windowFrame } from '@/lib/runtime'

/** Spread on a header, toolbar or side list: on macOS its empty space drags the window. */
export function dragRegion(): { 'data-tauri-drag-region'?: 'deep' } {
  return hasMacOverlayTitlebar() ? { 'data-tauri-drag-region': 'deep' } : {}
}

/**
 * Spread on a backdrop (the window frame, the lock screen): clicks on the
 * backdrop itself drag the window, clicks on anything in front of it do not.
 */
export function backdropDragRegion(): { 'data-tauri-drag-region'?: '' } {
  return windowFrame() === 'browser' ? {} : { 'data-tauri-drag-region': '' }
}
