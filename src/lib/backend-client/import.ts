import type { ImportBatchDetail, TakeoutInspection, TakeoutRequest } from '../types'
import { call } from './shared'

export const importClient = {
  inspectTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('inspect_takeout', { request }),
  importTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('import_takeout', { request }),
  previewBatch: (batchId: number) =>
    call<ImportBatchDetail>('preview_import_batch', { batchId }),
  revertBatch: (batchId: number) =>
    call<ImportBatchDetail>('revert_import_batch', { batchId }),
  restoreBatch: (batchId: number) =>
    call<ImportBatchDetail>('restore_import_batch', { batchId }),
}
