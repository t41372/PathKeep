/**
 * @file backend-preview-command-result.ts
 * @description Shared handled-or-unhandled contract for extracted browser-preview command modules.
 * @module lib/backend-preview-command-result
 *
 * ## Responsibilities
 * - Expose one canonical sentinel for preview command handlers that do not own a given command.
 * - Keep extracted command modules interoperable without relying on magic strings or ambiguous `undefined` results.
 * - Provide a tiny type guard so the main dispatcher can stay readable when delegating across modules.
 *
 * ## Not responsible for
 * - Dispatching any preview command on its own.
 * - Defining browser-preview state or route-level behavior.
 * - Encoding desktop transport behavior; `backend.ts` still decides when Tauri is active.
 *
 * ## Dependencies
 * - Has no runtime dependencies because every preview command module needs to import it cheaply.
 *
 * ## Performance notes
 * - This file stays minimal on purpose because every extracted preview command module imports it.
 */

/**
 * Marks a preview command as "not owned here" without colliding with valid command results such as `undefined` or `null`.
 *
 * Several preview commands legitimately return empty values, so a dedicated sentinel is safer than overloading
 * plain JavaScript falsy values.
 */
export const PREVIEW_COMMAND_UNHANDLED = Symbol(
  'pathkeep.preview-command-unhandled',
)

/**
 * Represents the two possible outcomes of one extracted preview command handler.
 *
 * A handler either returns a real typed command result or the shared sentinel to tell the main dispatcher
 * to keep asking the next command module.
 */
export type PreviewCommandResult<T> = T | typeof PREVIEW_COMMAND_UNHANDLED

/**
 * Narrows a delegated preview command result back to a handled branch.
 *
 * This keeps the main dispatcher readable and avoids repeating direct symbol comparisons all over `backend.ts`.
 */
export function isPreviewCommandHandled<T>(
  result: PreviewCommandResult<T>,
): result is T {
  return result !== PREVIEW_COMMAND_UNHANDLED
}
