/**
 * What the user did to conversations inside PathKeep: put one away (封存) or
 * deleted it from PathKeep. Neither touches the tool's own files.
 *
 * Put away = out of the lists, still kept, still found by search and still
 * counted in insights. Deleted = the content is gone from PathKeep, and the
 * next backup must not bring it back while the tool still has the file, so a
 * deletion is remembered (which tool, which session, when; no title, no content).
 *
 * In the product both are the user's own data and live in PathKeep's archive.
 * Here they are in memory and reset on reload.
 */
import { useSyncExternalStore } from 'react'
import { NOW } from '~/data/fixture'

interface State {
  archived: ReadonlySet<number>
  /** Deleted conversation → when it was deleted. */
  deleted: ReadonlyMap<number, number>
  /** The conversation whose delete dialog is open. */
  deleting: number | null
}

let state: State = { archived: new Set(), deleted: new Map(), deleting: null }
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

export function askDelete(id: number | null) {
  update({ deleting: id })
}

export function deleteChat(id: number) {
  const archived = new Set(state.archived)
  archived.delete(id)
  update({ archived, deleted: new Map(state.deleted).set(id, NOW), deleting: null })
}

/** Forget a deletion: the next backup stores the conversation again if the tool still has it. */
export function allowAgain(id: number) {
  const deleted = new Map(state.deleted)
  deleted.delete(id)
  update({ deleted })
}
