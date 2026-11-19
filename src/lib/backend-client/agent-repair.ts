/**
 * Typed client for repairing saved Agent conversations whose files are
 * damaged, and for making a safety copy of the archive on request.
 *
 * Why this file exists:
 * - The repair is its own Preview → Execute action. The backend decides which
 *   repair applies from what it can establish, prepares it, holds it for 15
 *   minutes under a token, and installs exactly that on execute.
 * - A safety copy can be made while conversations cannot be read, so browsing
 *   history keeps its protection; such a copy is marked.
 *
 * Responsible for: command names, request envelopes, wire shapes and the
 * refusal codes these commands answer with. Not responsible for: progress
 * and stopping (`maintenance.ts`: `get_maintenance_progress`, and
 * `cancel_maintenance`, which also discards a prepared repair), or any copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Protect
 * browsing and repair conversation storage), `vault_worker::data_kinds::repair`
 * and `vault_core::maintenance::ConversationRepairPreview`.
 */
import type { RecoverySnapshot } from '../types'
import { call } from './shared'

/**
 * What the repair does. `rebuild`: the saved conversations are intact and
 * their lists, search and insights are rebuilt from them. `fromCopy`: both
 * conversation files come from a listed safety copy. `startFresh`: new files,
 * with every saved version that still verifies brought back.
 */
export type RepairAction = 'rebuild' | 'fromCopy' | 'startFresh'

/** Where the repair takes conversations from: a listed copy, or (none) what is here. */
export interface RepairSource {
  /** A listed safety copy's `path`. */
  copyPath?: string
  /** Only for a copy under another password than the archive's. */
  copyPassword?: string
}

export interface RepairPreview {
  action: RepairAction
  /** The copy, for `fromCopy`; `null` otherwise. */
  copyPath: string | null
  /** The copy's operation (`RecoverySnapshot.sourceOp`), for `fromCopy`. */
  sourceOp: string | null
  /** When the copy was made, for `fromCopy`. */
  createdAt: string | null
  /** Conversations readable after the repair, a decimal string. */
  conversations: string
  /**
   * `startFresh` only, like the next two: of `conversations`, those that come
   * back with their latest saved version. A decimal string; `null` when the
   * damaged files don't show what the latest versions were. The actions that
   * don't compare leave all three `null`.
   */
  conversationsRecoveredWhole: string | null
  /**
   * Of `conversations`, those that come back only as an older saved version:
   * their latest messages stay in the kept copy of the damaged files.
   */
  conversationsRecoveredPartially: string | null
  /** Conversations the damaged files hold that don't come back at all; `null` when that can't be counted. */
  conversationsOnlyInCopy: string | null
  /**
   * Single details (a star, a title, which file a conversation came from)
   * that didn't verify and are left out. A decimal string, any action.
   */
  skippedOptionalRecords: string
  /** Whole lists of such details, or inventories, that couldn't be read at all. */
  unreadableOptionalSections: string
  /** Where what the repair replaces is kept, as a listed safety copy. */
  keptAs: string
  /** The repair ends this computer's folder approval: the folders are chosen again before the next backup. */
  reviewSourcesBeforeBackup: boolean
  /** The next backup reads every file still in the tools' folders again. */
  nextBackupRecapturesToolFiles: boolean
  /** Some conversations only the archive held, which no longer verify, stay readable only in the kept copy. */
  archiveOnlyConversationsUnreadable: boolean
  /** Binds the execute request; consumed by it, expired after 15 minutes, superseded by a later preview. */
  token: string
}

export interface RepairResult {
  /** The safety copy that holds what the repair replaced. */
  keptCopyPath: string
  availability: 'available'
}

export interface SafetyCopyPreview {
  /** The folder the copy goes into. */
  directory: string
  /** Saved conversations cannot be read now; the copy will be marked. */
  conversationsUnreadable: boolean
  previewToken: string
}

/** Move export, move import and changing encryption refuse with this while conversations cannot be read. */
export const CONVERSATION_REPAIR_REQUIRED = 'conversation-repair-required'

/**
 * The same operations, and the repair check, refuse with this while the
 * system refuses access to the conversation files for now: not damage, so
 * not repaired; "Try again" (`retry_agent_store`) once access is back.
 */
export const CONVERSATION_ACCESS_UNAVAILABLE = 'conversation-access-unavailable'

/** The refusals the repair and the safety copy answer with (`command_error.rs`). */
export const REPAIR_ERROR_CODES = [
  'conversation-repair-preview-stale',
  'conversation-preservation-failed',
  'conversation-access-unavailable',
  'conversation-copy-no-verified-evidence',
  'conversation-copy-not-listed',
  'conversation-copy-incomplete',
  'conversation-copy-invalid-seal',
  'conversation-copy-duplicate-role',
  'conversation-copy-unsafe-path',
  'conversation-schema-too-new',
  'conversation-repair-healthy',
  'conversation-evidence-readable',
  'conversation-pair-unverified',
  'snapshot-digest-mismatch',
  'archive-password-required',
  'archive-password-wrong',
  'archive-busy',
  'lock-required',
] as const
export type RepairErrorCode = (typeof REPAIR_ERROR_CODES)[number]

export const agentRepairClient = {
  /** Prepares the repair the backend chooses, or one from `copyPath`. Runs as a stoppable maintenance job. */
  preview: (source: RepairSource) =>
    call<RepairPreview>('preview_agent_history_repair', {
      request: source,
    }),
  /**
   * Installs the prepared repair. Only the copy and the token: the check
   * already opened the copy, so its password is not sent again.
   */
  repair: (copyPath: string | undefined, previewToken: string) =>
    call<RepairResult>('repair_agent_history', {
      request: copyPath ? { copyPath, previewToken } : { previewToken },
    }),
  /** What a safety copy made now would hold, and where it would go. Reads only. */
  previewSafetyCopy: () =>
    call<SafetyCopyPreview>('preview_archive_safety_copy', { request: {} }),
  /** Makes the copy the preview described; a changed archive needs a new preview. */
  createSafetyCopy: (previewToken: string) =>
    call<RecoverySnapshot>('create_archive_safety_copy', {
      request: { previewToken },
    }),
}
