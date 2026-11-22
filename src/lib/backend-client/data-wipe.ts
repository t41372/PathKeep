/**
 * "Delete all data": a preview of what would be removed, then the deletion.
 * The execute call requires the confirmation word the user typed. Also the
 * end of a deletion PathKeep 0.5 started and didn't finish, which the
 * recovery screen asks the user to confirm again.
 */
import { call } from './shared'

/**
 * The word the user types to delete, in every language: the backend accepts
 * only this exact string, so no translation can make a deletion fail or pass.
 */
export const WIPE_CONFIRMATION_WORD = 'DELETE'

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
  /**
   * Finishes the "Delete all data" PathKeep 0.5 left unfinished
   * (`archive-wipe-confirmation-required`). Its record doesn't prove it
   * belongs to this folder, so the backend won't finish it on its own; the
   * user confirms with the same word. Resolves whether there was one.
   */
  finishInterrupted: (confirmation: string) =>
    call<boolean>('confirm_interrupted_wipe', { confirmation }),
}
