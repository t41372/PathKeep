/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `supportClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import { call } from './shared'

/**
 * Exposes the focused client surface for support commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export const supportClient = {
  openPathInFileManager: (path: string) =>
    call<string>('open_path_in_file_manager', { path }),
  openExternalUrl: (url: string) => call<string>('open_external_url', { url }),
  /**
   * Writes a UTF-8 text document to a user-chosen path and resolves with the number of bytes
   * written. Used by the AI assistant's "Export conversation" affordance after the native save
   * dialog returns the target path — the same save-dialog → backend-write shape the Settings →
   * Data migration export uses, so the export stays transparent and never invents a new transport.
   */
  exportConversationFile: (targetPath: string, contents: string) =>
    call<number>('export_conversation_file', { targetPath, contents }),
  /**
   * Opens the app's local log directory in the OS file manager (Finder on
   * macOS, Explorer on Windows). Resolves with the logs folder path that was
   * revealed. Used by the "Reveal logs" diagnostics affordance so users can
   * attach log files to bug reports without hunting for the path manually.
   */
  revealLogs: () => call<string>('reveal_logs'),
}
