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
 *
 * ## Not responsible for
 * - Formatting errors for display (`lib/errors.ts` owns that).
 * - Deciding what remediation UI looks like (routes own that).
 */

/** Backend classification: the operation needs an unlock first. */
export const ERROR_CODE_LOCK_REQUIRED = 'lock-required'

/** Backend classification: macOS Full Disk Access (TCC) is missing. */
export const ERROR_CODE_FULL_DISK_ACCESS = 'full-disk-access'

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
      code?: string | null
      actionHint?: string | null
      retryHint?: string | null
    },
  ) {
    super(message)
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
