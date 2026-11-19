/**
 * Which "Before …" label a safety copy gets, from the operation it was made
 * before (`RecoverySnapshot.sourceOp`). Shared by the restore list, Free up
 * space and the conversation repair's list of copies, so a copy is called the
 * same in all three. `manual` is a copy the person made; `agentRepair` holds
 * what a conversation repair replaced; `heldGeneration` is a conversation
 * file PathKeep kept when it disappeared while open (conversations only, so
 * the restore list never shows it).
 */
/** A conversation file kept on its own: no browsing history, so only a repair can use it. */
export const HELD_COPY = 'heldGeneration'

const knownOps = [
  'rekey',
  'reconcile',
  'import',
  'restore',
  'periodic',
  'manual',
  'agentRepair',
  'heldGeneration',
] as const

/** The `settingsStorage.restore.op.*` key for `op`; `unknown` for any other. */
export function copyLabelKey(op: string) {
  return (knownOps as readonly string[]).includes(op)
    ? (op as (typeof knownOps)[number])
    : 'unknown'
}
