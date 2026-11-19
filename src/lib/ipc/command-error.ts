/**
 * Structured desktop command error contract shared by every route.
 *
 * ## Responsibilities
 * - Define the typed error the IPC bridge throws when a desktop command fails
 *   (`code` / `actionHint` / `retryHint` + full diagnostic `message`), mirroring
 *   the backend `CommandError` envelope required by `module-boundary-map.md`.
 * - Provide the ONLY classification helpers routes may use to detect
 *   remediation cases (Full Disk Access, lock-required). Routes must never
 *   sniff message text themselves — and translated copy must never be fed
 *   back into an error channel for re-classification.
 * - Read the file a refusal names (`affectedFile`), which the bridge keeps
 *   on the error as the raw envelope (`cause`).
 *
 * ## Not responsible for
 * - Formatting errors for display (`lib/errors.ts` owns that).
 * - Deciding what remediation UI looks like (routes own that).
 */

/** Backend classification: the operation needs an unlock first. */
export const ERROR_CODE_LOCK_REQUIRED = 'lock-required'

/** Backend classification: the archive is not open in this session. */
export const ERROR_CODE_ARCHIVE_PASSWORD_REQUIRED = 'archive-password-required'

/** Backend classification: the password given for the archive does not open it. */
export const ERROR_CODE_ARCHIVE_PASSWORD_WRONG = 'archive-password-wrong'

/** Backend classification: macOS Full Disk Access (TCC) is missing. */
export const ERROR_CODE_FULL_DISK_ACCESS = 'full-disk-access'

/** Backend classification: writing a destination file failed (`affectedFile` names it). */
export const ERROR_CODE_ARCHIVE_FILE_WRITE_FAILED = 'archive-file-write-failed'

/**
 * What a named file is to the archive: one of the browsing files (or a file
 * of the archive as a whole, such as a copy's seal), the file of saved
 * conversations, the file of their lists, or a conversation file PathKeep
 * kept when it went missing. A later backend may add roles.
 */
export type AffectedFileRole =
  'browsing' | 'evidence' | 'readModel' | 'heldGeneration'

/** The archive member a refusal is about (`CommandError.affectedFile`). */
export interface AffectedFile {
  /** A local path; it can name a companion (`-wal`) or a file that is gone. */
  path: string
  role: AffectedFileRole | (string & {})
}

/**
 * Error thrown by `invokeCommand` when the desktop backend rejects a command.
 *
 * `message` carries the full backend cause chain (diagnostic content — the
 * user is also the bug reporter). The structured fields carry the backend's
 * classification so frontend behavior never depends on message wording.
 */
export class CommandInvokeError extends Error {
  readonly code: string | null
  readonly actionHint: string | null
  readonly retryHint: string | null

  constructor(
    message: string,
    options?: {
      cause?: unknown
      code?: string | null
      actionHint?: string | null
      retryHint?: string | null
    },
  ) {
    super(message, { cause: options?.cause })
    this.name = 'CommandInvokeError'
    this.code = options?.code ?? null
    this.actionHint = options?.actionHint ?? null
    this.retryHint = options?.retryHint ?? null
  }
}

/**
 * Reads the backend classification code from any thrown value, or `null` when
 * the value carries none (plain Errors, preview-fixture failures, strings).
 *
 * Only two shapes are trusted as classification sources: the bridge's
 * `CommandInvokeError`, and a raw backend envelope (`code` + string
 * `message`/`error`) for callers that receive an un-wrapped rejection. An
 * arbitrary object that merely happens to have a `code` field (e.g. a DOM or
 * plugin error with a numeric-ish string code) is NOT treated as an envelope.
 */
export function commandErrorCode(error: unknown): string | null {
  if (error instanceof CommandInvokeError) {
    return error.code
  }
  if (typeof error === 'object' && error !== null) {
    const record = error as Record<string, unknown>
    const hasEnvelopeMessage =
      typeof record.message === 'string' || typeof record.error === 'string'
    if (hasEnvelopeMessage && typeof record.code === 'string') {
      return record.code
    }
  }
  return null
}

/**
 * The file a refusal names, or `null` when it names none (a refusal about
 * the plan as a whole, such as an expired check). Reads the raw envelope:
 * the one a `CommandInvokeError` carries as its `cause`, or an un-wrapped one.
 */
export function commandErrorAffectedFile(error: unknown): AffectedFile | null {
  const envelope = error instanceof CommandInvokeError ? error.cause : error
  if (typeof envelope !== 'object' || envelope === null) return null
  const file = (envelope as Record<string, unknown>).affectedFile
  if (typeof file !== 'object' || file === null) return null
  const { path, role } = file as Record<string, unknown>
  return typeof path === 'string' && path.length > 0 && typeof role === 'string'
    ? { path, role }
    : null
}

/** Whether PathKeep failed to write a destination file. */
export function isArchiveFileWriteFailed(error: unknown): boolean {
  return commandErrorCode(error) === ERROR_CODE_ARCHIVE_FILE_WRITE_FAILED
}

/**
 * Returns `true` when a RAW backend message carries the Full Disk Access
 * marker the backend guarantees (`browser_access.rs` routes every macOS TCC
 * denial into copy containing the ASCII `"Full Disk Access"` marker; the
 * ingest skip path still produces the legacy Safari-specific variant).
 *
 * Only for backend-authored string channels such as `BackupReport.warnings`.
 * Never call this with localized copy — translated text must never re-enter
 * error classification.
 */
export function isFullDiskAccessIssueMessage(message: string): boolean {
  return (
    message.includes('Full Disk Access') ||
    message.includes('Safari History.db is not readable yet')
  )
}

/**
 * Returns `true` when a thrown value is the backend's Full Disk Access
 * refusal. Prefers the structured `code`; falls back to the raw backend
 * marker for channels that predate the envelope (frozen preview fixtures).
 */
export function isFullDiskAccessError(error: unknown): boolean {
  const code = commandErrorCode(error)
  if (code !== null) {
    return code === ERROR_CODE_FULL_DISK_ACCESS
  }
  return error instanceof Error && isFullDiskAccessIssueMessage(error.message)
}

/**
 * Returns `true` when a thrown value means the archive is encrypted/locked
 * and the operation needs an unlock. The shell's unlock gate is the
 * remediation surface for these — no toast.
 *
 * Prefers the structured `code`; the fallback matches only the two marker
 * strings the backend actually produces (`archive::schema` /
 * `source_evidence`'s "database key is required" and `app_lock.rs`'s
 * "PathKeep is currently locked") for pre-envelope channels.
 */
export function isLockRequiredError(error: unknown): boolean {
  const code = commandErrorCode(error)
  if (code !== null) {
    return code === ERROR_CODE_LOCK_REQUIRED
  }
  if (!(error instanceof Error)) return false
  return (
    error.message.includes('database key is required') ||
    error.message.includes('PathKeep is currently locked')
  )
}

/**
 * Returns `true` when a read failed because the archive is closed: the app is
 * locked, or the archive is not open in this session (a fresh session, or
 * maintenance that closed it). The shell's gates are the remedy: unlock from
 * the keychain, or ask for the password or passcode.
 */
export function isArchiveClosedError(error: unknown): boolean {
  return (
    isLockRequiredError(error) ||
    commandErrorCode(error) === ERROR_CODE_ARCHIVE_PASSWORD_REQUIRED
  )
}
