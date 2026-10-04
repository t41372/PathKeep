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
  /** True when an automatic backup is installed; the deletion removes it after the files. */
  removesSchedule: boolean
  /** What the scheduler reports installed: the LaunchAgent file or the Task Scheduler task. */
  scheduleItems: string[]
}

/** What the deletion did besides deleting the files, which it always did when this returns. */
export interface WipeReport {
  scheduleRemoved: boolean
  /** Why the installed automatic backup could not be removed; it is still installed. */
  scheduleError: string | null
}

export const dataWipeClient = {
  preview: () => call<WipePreview>('preview_wipe_all_data'),
  /** `confirmation` must be the literal string "DELETE". */
  execute: (confirmation: string) =>
    call<WipeReport>('wipe_all_data', { confirmation }),
}
