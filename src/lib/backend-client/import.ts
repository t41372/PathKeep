/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `importClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type {
  BrowserHistoryImportRequest,
  ImportBatchDetail,
  TakeoutInspection,
  TakeoutRequest,
} from '../types'
import { call } from './shared'

/**
 * Exposes the focused client surface for import commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export const importClient = {
  inspectTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('inspect_takeout', { request }),
  importTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('import_takeout', { request }),
  inspectBrowserHistory: (request: BrowserHistoryImportRequest) =>
    call<TakeoutInspection>('inspect_browser_history', { request }),
  importBrowserHistory: (request: BrowserHistoryImportRequest) =>
    call<TakeoutInspection>('import_browser_history', { request }),
  previewBatch: (batchId: number) =>
    call<ImportBatchDetail>('preview_import_batch', { batchId }),
  revertBatch: (batchId: number) =>
    call<ImportBatchDetail>('revert_import_batch', { batchId }),
  restoreBatch: (batchId: number) =>
    call<ImportBatchDetail>('restore_import_batch', { batchId }),
}
