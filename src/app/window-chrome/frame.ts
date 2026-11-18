/**
 * The room a screen with a main panel (the archive's frame, onboarding)
 * leaves at the top for the window's frame. One rule for every such screen,
 * so the window buttons sit in the same strip of window background wherever
 * the window is.
 *
 * Responsible for: that margin. Not responsible for: the buttons
 * (`window-controls.tsx`) or what drags the window (`drag-region.ts`).
 */
import { drawsWindowControls } from '@/lib/runtime'

/**
 * The margin above a main panel: on Windows and Linux a 32 px strip for the
 * window buttons (they are 32 px tall), elsewhere the gap the panel keeps on
 * every side.
 */
export function panelTopMargin() {
  return drawsWindowControls() ? 'mt-8' : 'mt-2.5'
}
