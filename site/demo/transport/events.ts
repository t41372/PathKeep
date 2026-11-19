/**
 * Stands in for `src/lib/ipc/events.ts` in the demo build. Nothing runs in
 * the background of the demo, so nothing sends events.
 */
import type * as Original from '@/lib/ipc/events'

export const listenDesktopEvent: typeof Original.listenDesktopEvent =
  async () => () => {}
