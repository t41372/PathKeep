/**
 * "Delete all data": a preview of what would be removed, then the deletion.
 * The execute call requires the confirmation word the user typed.
 */
import { call } from './shared'

export interface WipePreview {
  /** Every path that will be deleted, with its size on disk. */
  items: { path: string; bytes: number }[]
  totalBytes: number
  visitCount: number
  /** True when an archive key is stored in the system keychain and will be removed too. */
  clearsKeychain: boolean
}

export const dataWipeClient = {
  preview: () => call<WipePreview>('preview_wipe_all_data'),
  /** `confirmation` must be the literal string "DELETE". */
  execute: (confirmation: string) => call<void>('wipe_all_data', { confirmation }),
}
