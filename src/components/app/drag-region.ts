/**
 * Where the window can be dragged from, as from any title bar. On macOS the
 * window has no title bar of its own (the traffic lights float over the
 * content), so PathKeep marks its own: a strip across the top of every
 * scrolling screen, which stays put while the content scrolls under it, and the
 * empty parts of headers, toolbars and side lists. Buttons, fields and links
 * inside a marked area still work: Tauri does not start a drag from them.
 * Elsewhere the window keeps the system title bar, so nothing is marked.
 *
 * Responsible for: those marks. Not responsible for: the window or its
 * traffic lights.
 */
import { hasMacOverlayTitlebar } from '@/lib/runtime'

/** Spread on a header, toolbar or side list: its empty space drags the window. */
export function dragRegion(): { 'data-tauri-drag-region'?: 'deep' } {
  return hasMacOverlayTitlebar() ? { 'data-tauri-drag-region': 'deep' } : {}
}

/**
 * Spread on a backdrop (the window frame, the lock screen): clicks on the
 * backdrop itself drag the window, clicks on anything in front of it do not.
 */
export function backdropDragRegion(): { 'data-tauri-drag-region'?: '' } {
  return hasMacOverlayTitlebar() ? { 'data-tauri-drag-region': '' } : {}
}
