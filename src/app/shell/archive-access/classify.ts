/**
 * What the recovery screen says and offers for each answer of
 * `get_archive_access`: the backend's state and code, turned into one kind
 * of screen.
 *
 * Responsibilities: map every (state, code) pair the backend can return
 * (`src-tauri/src/archive_access.rs`) to a kind, and say which file a
 * missing-store answer names.
 *
 * Not responsible for: the words (the screen's catalog) or the actions.
 */
import type { ArchiveAccess } from '@/lib/backend-client/archive-access'
import type { RecoverySnapshot } from '@/lib/types'

/** Which file of a PathKeep 0.5 archive is missing, read from the path the backend names. */
export type MissingStore = 'ask' | 'intelligence' | 'evidence' | 'other'

export type AccessKind =
  | { kind: 'newer'; needed: number | null }
  | { kind: 'writer'; needed: number | null }
  | { kind: 'unreadable' }
  | { kind: 'controlMissing' }
  | { kind: 'controlInvalid' }
  | { kind: 'installRecord' }
  | { kind: 'storeMissing'; store: MissingStore }
  | { kind: 'handoff' }
  | { kind: 'handoffInvalid' }
  | { kind: 'legacyWipe' }
  | { kind: 'interrupted' }
  | { kind: 'recoveryMode' }
  | { kind: 'unknown' }

/**
 * The backend names the missing file only by its path (the access report has
 * no field for which store), so the path's last two parts decide.
 */
export function missingStore(detail: string | null): MissingStore {
  const path = (detail ?? '').replaceAll('\\', '/')
  if (path.includes('derived/agent.db')) return 'ask'
  if (path.includes('derived/intelligence.db')) return 'intelligence'
  if (path.includes('archive/source-evidence.db')) return 'evidence'
  return 'other'
}

export function classify(access: ArchiveAccess): AccessKind {
  switch (access.code) {
    case 'archive-reader-too-old':
      return { kind: 'newer', needed: access.archive?.minReader ?? null }
    case 'archive-config-format-too-new':
    case 'archive-schema-too-new':
      return { kind: 'newer', needed: null }
    case 'archive-writer-too-old':
      return { kind: 'writer', needed: access.archive?.minWriter ?? null }
    case 'archive-control-unreadable':
      return { kind: 'unreadable' }
    case 'archive-control-missing':
      return { kind: 'controlMissing' }
    case 'archive-control-invalid':
    case 'archive-compatibility-invalid':
      // Refused before the desktop started: the settings file itself. Found
      // only as the archive opened: a record of an unfinished change that
      // doesn't fit, which a rebuilt settings file would not fix.
      return access.recoveryOnly
        ? { kind: 'controlInvalid' }
        : { kind: 'installRecord' }
    case 'archive-store-missing':
      return { kind: 'storeMissing', store: missingStore(access.detail) }
    case 'archive-handoff-choice-required':
      return { kind: 'handoff' }
    case 'archive-handoff-invalid':
      return { kind: 'handoffInvalid' }
    case 'archive-wipe-confirmation-required':
      return { kind: 'legacyWipe' }
    case 'archive-operation-interrupted':
      return { kind: 'interrupted' }
    case null:
      return access.recoveryOnly
        ? { kind: 'recoveryMode' }
        : { kind: 'unknown' }
    default:
      return { kind: 'unknown' }
  }
}

/**
 * Kinds whose archive a safety copy can take back: the copies list is
 * offered there. Not for an archive a newer PathKeep changed (updating
 * fixes it), an unreadable folder, an unresolved handoff (nothing restores
 * until it is chosen) or 0.5's unfinished delete.
 */
export function offersRestore(kind: AccessKind['kind']) {
  return [
    'controlMissing',
    'controlInvalid',
    'installRecord',
    'storeMissing',
    'interrupted',
    'recoveryMode',
    'unknown',
  ].includes(kind)
}

/**
 * Copies that bring browsing history back: not conversation-only ones (a
 * file kept when a conversation file went missing, an undone import's
 * conversations). A copy listed without its kinds is an old-format one,
 * which holds browsing history.
 */
export function restorable(copy: RecoverySnapshot) {
  return (
    copy.verifiedOpenable &&
    (copy.dataKinds.length === 0 || copy.dataKinds.includes('browsing'))
  )
}
