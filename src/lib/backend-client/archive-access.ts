/**
 * Typed client for whether this build may open the archive in its data
 * folder, and for the few commands that work when it may not.
 *
 * Why this file exists:
 * - Startup asks `get_archive_access` before anything else. A refused or
 *   damaged archive starts the desktop in a restricted mode where only the
 *   recovery commands answer (`archive_access::recovery_command`); every
 *   other command returns `archive-recovery-only`.
 * - The previous-version handoff (prepare for PathKeep 0.5.0, then take the
 *   conversations back or start empty on the next launch), the folder-volume
 *   check and the one-time notices after an undone "Delete all data" share
 *   that startup path.
 *
 * Source-of-truth notes:
 * - Contract: `docs/architecture/desktop-command-surface.md` § "Group 5
 *   archive recovery commands" and `src-tauri/src/archive_access.rs`.
 * - Every call sends an argument object, even an empty one: the dev bridge
 *   parses a struct for each command.
 */
import { call } from './shared'

/** What the startup check decided. */
export type ArchiveAccessState =
  'open' | 'notInitialized' | 'refusedRead' | 'refusedWrite' | 'needsRecovery'

/** The three numbers an archive's control declares about itself. */
export interface ArchiveStamp {
  revision: number
  minReader: number
  minWriter: number
}

export interface ArchiveAccess {
  state: ArchiveAccessState
  /**
   * The desktop started without opening the archive: only the recovery
   * commands work, and a repair needs a restart before the archive opens.
   */
  recoveryOnly: boolean
  /** Why, as a backend code (`archive-reader-too-old`, …); null when open. */
  code: string | null
  /** The backend's own words, in English, for the person to copy. */
  detail: string | null
  /** The data folder. */
  root: string
  /** The newest archive revision this build reads and writes. */
  buildLevel: number
  /** What the archive's control declares, when it could be read. */
  archive: ArchiveStamp | null
}

/** Which part of a damaged control or a 0.5 archive to rebuild. */
export type ArchiveControlRepair = 'settings' | 'ask' | 'intelligence'

/** What preparing for the previous version moves and keeps. */
export interface ArchiveRollbackPreview {
  /** The release the prepared archive is for (`0.5.0`). */
  targetVersion: string
  /**
   * Before preparing: the folder the package will be made in (next to the
   * archive). Once prepared: the package itself.
   */
  packageFolder: string
  encrypted: boolean
  /** The two Agent conversation files that move into the package. */
  conversationBytes: number
  /**
   * The copies and move files this version made that move with them,
   * relative to the data folder.
   */
  copyFoldersAndFiles: string[]
  /** Space taken back into the archive when the conversations rejoin it. */
  rejoinCopyBytes: number
  /** The archive was prepared earlier: the person chooses before it opens. */
  needsChoice: boolean
}

/** A one-time notice about the data folder, drained by `takeWipeNotices`. */
export interface RecoveryNotice {
  /**
   * `wipe-rolled-back`, `wipe-record-discarded`, `wipe-files-displaced`,
   * `archive-check-disk`; a later backend may add codes.
   */
  code: string
  path: string | null
}

export interface StorageVolume {
  folder: string
  /** FAT32 or exFAT: a power cut while PathKeep writes can lose a file. */
  fat: boolean
}

export const archiveAccessClient = {
  get: () => call<ArchiveAccess>('get_archive_access', {}),
  /** `currentPassword` is the archive's own password, never a copy's. */
  repairControl: (
    repair: ArchiveControlRepair,
    currentPassword: string | null,
  ) => call<null>('repair_archive_control', { repair, currentPassword }),
  /** Starts the desktop again so the ordinary owner opens the repaired archive. */
  restart: () => call<null>('restart_after_archive_recovery', {}),
  previewRollback: () =>
    call<ArchiveRollbackPreview>('preview_archive_rollback', {}),
  /** Resolves to the package folder. The running app is then recovery-only. */
  prepareRollback: (currentPassword: string | null) =>
    call<string>('prepare_archive_rollback', { currentPassword }),
  /**
   * `rejoin` takes the set-aside conversations back; false starts with none
   * and leaves the package where it is. `packagePassword` is the password the
   * conversations had when they were set aside.
   */
  resolveHandoff: (
    rejoin: boolean,
    currentPassword: string | null,
    packagePassword: string | null,
  ) =>
    call<null>('resolve_archive_handoff', {
      rejoin,
      currentPassword,
      packagePassword,
    }),
  /** The data folder's volume when `folder` is null. */
  storageVolume: (folder: string | null = null) =>
    call<StorageVolume>('get_storage_volume', { folder }),
  /** Each notice is returned once; the next call no longer has it. */
  takeWipeNotices: () => call<RecoveryNotice[]>('take_wipe_notice', {}),
}
