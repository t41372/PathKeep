/**
 * Which project dialog is open (rename, project settings, add a folder). One
 * host per page renders them, so the projects page and a project's own header
 * open the very same dialogs from the same menu.
 */
import { useSyncExternalStore } from 'react'

export type DialogKind = 'rename' | 'settings' | 'add-folder'
let open: { kind: DialogKind; id: string } | null = null
const listeners = new Set<() => void>()

export function openProjectDialog(kind: DialogKind, id: string) {
  open = { kind, id }
  for (const l of listeners) l()
}

export function closeProjectDialog() {
  open = null
  for (const l of listeners) l()
}

export function useProjectDialog() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => open,
  )
}
