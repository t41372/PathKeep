/**
 * Which "Before …" label a safety copy gets, from the operation it was made
 * before (`RecoverySnapshot.sourceOp`). Shared by the restore list and Free up
 * space, so a copy is called the same in both.
 */
const knownOps = [
  'rekey',
  'reconcile',
  'import',
  'restore',
  'periodic',
] as const

/** The `settingsStorage.restore.op.*` key for `op`; `unknown` for any other. */
export function copyLabelKey(op: string) {
  return (knownOps as readonly string[]).includes(op)
    ? (op as (typeof knownOps)[number])
    : 'unknown'
}
