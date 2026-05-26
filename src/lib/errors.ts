/**
 * Centralised error → user-displayable string formatter.
 *
 * ## Responsibilities
 * - Convert any `unknown` thrown value (Error, string, Tauri plugin error
 *   object, anyhow-style object with `message`/`error`, primitives) into
 *   the most informative string we can produce.
 * - Preserve underlying detail rather than hiding it behind an i18n
 *   "unknown reason" placeholder — PathKeep is a local-only app and the
 *   user is *also* the bug reporter, so the real error matters.
 *
 * ## Not responsible for
 * - Localising the error itself. Error messages from the Tauri/Rust side
 *   already arrive as English-or-better strings; wrapping them in another
 *   i18n indirection just loses information.
 * - Stripping PII. There is no "server" — the user already owns the bytes.
 *
 * ## Why this exists
 * Routes used to write `error instanceof Error ? error.message : t('common.notAvailable')`,
 * which silently swallowed every plugin error, every Promise rejection
 * whose reason was a plain object, and every raw string Tauri throws when
 * a Rust command returns `Err(String)`. The result was banner copy like
 * "Export failed for an unknown reason." even when the backend produced a
 * perfectly actionable error chain.
 */

const MAX_DESCRIBED_LENGTH = 2_000

function asNonEmptyString(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : null
}

function describeObject(value: object): string | null {
  const record = value as Record<string, unknown>

  const messageField = asNonEmptyString(record.message)
  if (messageField) return messageField

  const errorField = asNonEmptyString(record.error)
  if (errorField) return errorField

  const reasonField = asNonEmptyString(record.reason)
  if (reasonField) return reasonField

  const descriptionField = asNonEmptyString(record.description)
  if (descriptionField) return descriptionField

  try {
    const json = JSON.stringify(value)
    if (json && json !== '{}' && json !== 'null') {
      return json
    }
  } catch {
    // Cyclic / non-serialisable — fall through to the type label.
  }

  return null
}

/**
 * Returns the best user-displayable description of an arbitrary thrown
 * value. Always returns a non-empty string.
 *
 * `context` (optional) is prepended when the helper could not extract any
 * useful information from the raw value — so the user at least knows
 * which operation failed.
 */
export function describeError(error: unknown, context?: string): string {
  if (error instanceof Error) {
    const message = asNonEmptyString(error.message)
    if (message) return clip(message)
    // Error instance with empty message — fall back to the class name so
    // the user can at least quote it back in a bug report.
    return clip(`${error.name || 'Error'}${context ? `: ${context}` : ''}`)
  }

  const direct = asNonEmptyString(error)
  if (direct) return clip(direct)

  if (typeof error === 'object' && error !== null) {
    const described = describeObject(error)
    if (described) return clip(described)
  }

  if (error === null) {
    return clip(context ? `${context}: null` : 'null')
  }
  if (error === undefined) {
    return clip(context ? `${context}: undefined` : 'undefined')
  }

  // Primitives (number, boolean, bigint, symbol). Avoid `String(error)` on
  // bare objects because eslint flags the default `[object Object]` stringify
  // path — we already handled the object case above with describeObject.
  if (typeof error !== 'object' && typeof error !== 'function') {
    return clip(String(error as number | boolean | bigint | symbol))
  }

  // Last resort — describe the shape so the user can quote it back.
  const typeLabel = typeof error
  return clip(context ? `${context}: <${typeLabel}>` : `<${typeLabel}>`)
}

function clip(value: string): string {
  if (value.length <= MAX_DESCRIBED_LENGTH) return value
  return `${value.slice(0, MAX_DESCRIBED_LENGTH - 1)}…`
}
