/**
 * What the user changed about projects: names, and which folders belong to
 * which project. A PathKeep project can hold several folders (like a Codex
 * desktop project with several paths); adding a folder that showed up as its
 * own project moves its conversations into this one. In the product these
 * live in PathKeep's own archive (never in the harness folders); here they
 * are in memory and reset on reload.
 */
import { useSyncExternalStore } from 'react'
import { archive } from '~/data/archive'
import type { MergeSuggestion } from '~/data/types'

/** What the user said about a "this folder may belong to that project" suggestion. */
export type Decision = 'added' | 'declined' | 'later'

interface State {
  names: Map<string, string>
  decisions: Map<string, Decision>
  /** Folders (shown as projects) the user added to another project by hand: from id → into id. */
  manual: Map<string, string>
}

let state: State = {
  names: new Map(),
  decisions: new Map(),
  manual: new Map(),
}
const listeners = new Set<() => void>()

function update(next: Partial<State>) {
  state = { ...state, ...next }
  for (const l of listeners) l()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useProjectEdits() {
  return useSyncExternalStore(subscribe, () => state)
}

export function rename(id: string, name: string) {
  const names = new Map(state.names)
  if (name.trim()) names.set(id, name.trim())
  else names.delete(id)
  update({ names })
}

export function decide(suggestionId: string, decision: Decision | null) {
  const decisions = new Map(state.decisions)
  if (decision) decisions.set(suggestionId, decision)
  else decisions.delete(suggestionId)
  update({ decisions })
}

/** Add `fromId`'s folders to `intoId`; `intoId = null` makes them a project of their own again. */
export function addFoldersTo(fromId: string, intoId: string | null) {
  const manual = new Map(state.manual)
  if (intoId) manual.set(fromId, intoId)
  else manual.delete(fromId)
  // A suggestion that already did this would otherwise keep it joined.
  const decisions = new Map(state.decisions)
  if (!intoId) for (const s of archive.suggestions) if (s.fromId === fromId && decisions.get(s.id) === 'added') decisions.delete(s.id)
  update({ manual, decisions })
}

/** Projects whose folders now belong to another project: from id → into id. */
export function joinedInto(s: State): Map<string, string> {
  const out = new Map<string, string>(s.manual)
  for (const sug of archive.suggestions) {
    if (s.decisions.get(sug.id) === 'added') out.set(sug.fromId, sug.intoId)
  }
  return out
}

export function openSuggestions(s: State): MergeSuggestion[] {
  return archive.suggestions.filter((sug) => {
    const d = s.decisions.get(sug.id)
    return d === undefined || d === 'later'
  })
}

export function displayName(s: State, id: string) {
  return s.names.get(id) ?? archive.projectById.get(id)?.name ?? id
}
