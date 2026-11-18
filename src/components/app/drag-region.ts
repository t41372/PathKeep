/**
 * Where the window can be dragged from, as from any title bar. A desktop
 * window has no title bar of the system's on any platform (`windowFrame`), so
 * PathKeep marks its own:
 * - The window background, on every platform: the margin around the main
 *   panel (on Windows and Linux, the strip beside the window buttons), the
 *   empty parts of the nav rail and the onboarding sidebar, and the backdrop
 *   of the lock, start and error screens.
 * - On macOS also inside the panel, whose top lies in the title strip the
 *   traffic lights float over: a strip across the top of every scrolling
 *   screen (`title-bar.tsx`), which stays put while the content scrolls under
 *   it, and the empty parts of headers, toolbars and side lists. On Windows
 *   and Linux the panel starts below the strip, and inside it is content.
 *
 * Buttons, fields and links inside a marked area still work: no drag starts
 * from them. A double click on a mark maximizes or restores the window. In a
 * browser nothing is marked.
 *
 * On macOS the marks are Tauri's (`data-tauri-drag-region`), which start the
 * drag on the press. On Windows and Linux they are PathKeep's own
 * (`WINDOW_DRAG_MARK`), which start it once the pointer moves, so a tap
 * cannot leave the window following the pointer (`window-chrome/drag.ts`).
 *
 * Responsible for: those marks. Not responsible for: the window or its
 * buttons, or carrying out the drag.
 */
import { hasMacOverlayTitlebar, windowFrame } from '@/lib/runtime'

/** The mark of a drag spot on Windows and Linux (`window-chrome/drag.ts`). */
export const WINDOW_DRAG_MARK = 'data-window-drag'

/** Spread on a header, toolbar or side list: on macOS its empty space drags the window. */
export function dragRegion(): { 'data-tauri-drag-region'?: 'deep' } {
  return hasMacOverlayTitlebar() ? { 'data-tauri-drag-region': 'deep' } : {}
}

/**
 * Spread on a backdrop (the window frame, the lock screen) or an empty spacer
 * on it (in the nav rail, the onboarding sidebar): clicks on the element
 * itself drag the window, clicks on anything in front of it do not.
 */
export function backdropDragRegion():
  | { 'data-tauri-drag-region': '' }
  | { [WINDOW_DRAG_MARK]: '' }
  | Record<string, never> {
  switch (windowFrame()) {
    case 'browser':
      return {}
    case 'mac-overlay':
      return { 'data-tauri-drag-region': '' }
    case 'drawn':
      return { [WINDOW_DRAG_MARK]: '' }
  }
}
