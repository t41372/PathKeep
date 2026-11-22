/**
 * Which "Before …" label a safety copy gets, from the operation it was made
 * before (`RecoverySnapshot.sourceOp`). Shared by the restore list, Free up
 * space and the conversation repair's list of copies, so a copy is called the
 * same in all three. `manual` is a copy the person made; `agentRepair` holds
 * what a conversation repair replaced; `heldGeneration` is a conversation
 * file PathKeep kept when it disappeared while open (conversations only, so
 * the restore list never shows it). Three more are listed only by Free up
 * space, since nothing restores from them: `importUndo` holds the
 * conversations an undone import added, `wipeTrash` what an undone "Delete
 * all data" left, `previousArchive` an archive an earlier version set aside.
 */
/** A conversation file kept on its own: no browsing history, so only a repair can use it. */
export const HELD_COPY = 'heldGeneration'

/** Kept aside, not a restore point: the conversations an undone import added. */
export const IMPORT_UNDO_COPY = 'importUndo'

/** Kept aside, not restore points: listed so they can be found and freed. */
export const SET_ASIDE_FOLDERS = ['wipeTrash', 'previousArchive']

const knownOps = [
  'rekey',
  'reconcile',
  'import',
  'restore',
  'periodic',
  'manual',
  'agentRepair',
  'heldGeneration',
  'importUndo',
  'wipeTrash',
  'previousArchive',
] as const

/** The `settingsStorage.restore.op.*` key for `op`; `unknown` for any other. */
export function copyLabelKey(op: string) {
  return (knownOps as readonly string[]).includes(op)
    ? (op as (typeof knownOps)[number])
    : 'unknown'
}
