/**
 * The event the backend sends after the stored choices about kinds of data
 * changed (a kind turned on, paused or off, an Agent tool's switch, reading
 * `.git`), from this window or anywhere else. A change that changed nothing
 * sends nothing.
 *
 * Not responsible for: what to refetch (`src/lib/queries/data-kinds.ts`).
 */
import {
  DATA_KINDS_CHANGED_EVENT,
  type DataKindsChanged,
} from '../backend-client/data-kinds'
import { listenDesktopEvent } from './events'

export function subscribeToDataKindsChanged(
  listener: (event: DataKindsChanged) => void,
) {
  return listenDesktopEvent<DataKindsChanged>(
    DATA_KINDS_CHANGED_EVENT,
    listener,
  )
}
