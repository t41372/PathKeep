/**
 * What the user did to conversations inside PathKeep: put one away (封存) or
 * moved it to the trash. Neither touches the tool's own files, and neither
 * removes anything from PathKeep's archive.
 *
 * Put away = out of the lists, still kept, still found by search and still
 * counted in insights. Trashed = out of every list, search result and insight,
 * whatever its put-away state; it stays in the archive and is backed up as
 * usual, and only the 垃圾桶 filter shows it. Putting it back leaves its
 * put-away state as it was.
 *
 * In the product both are the user's own data and live in PathKeep's archive.
 * Here they are in memory and reset on reload.
 */
import { useSyncExternalStore } from 'react'
import { NOW } from '~/data/fixture'

interface State {
  archived: ReadonlySet<number>
  /** Trashed conversation → when it went to the trash. */
  trashed: ReadonlyMap<number, number>
}

let state: State = { archived: new Set(), trashed: new Map() }
const listeners = new Set<() => void>()

function update(next: Partial<State>) {
  state = { ...state, ...next }
  for (const l of listeners) l()
}

export function useChatEdits() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => state,
  )
}

export function setArchived(id: number, on: boolean) {
  const archived = new Set(state.archived)
  if (on) archived.add(id)
  else archived.delete(id)
  update({ archived })
}

export function setTrashed(id: number, on: boolean) {
  const trashed = new Map(state.trashed)
  if (on) trashed.set(id, NOW)
  else trashed.delete(id)
  update({ trashed })
}
