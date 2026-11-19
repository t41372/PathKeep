/**
 * Damaging saved Agent conversations on disk, for the repair specs, the way a
 * failing disk would: while no PathKeep holds the files, and seen by an app
 * that opens them afresh.
 *
 * How the files are let go of (`withArchiveStopped`):
 * 1. The window leaves the app (`about:blank`), so it asks nothing meanwhile.
 * 2. A fault at the point where an opening archive joins its conversation
 *    files (`browsing-group-open`, debug builds) stops every open from
 *    finishing until it is lifted.
 * 3. The session key is cleared, which closes the archive cleanly: pages
 *    written out of the log, the conversation pair sealed.
 * 4. The app is killed; the launcher starts it again, and the new app's own
 *    open at start stops at the fault. A keychain unlock asked for then
 *    waits for that attempt and ends at the fault too. (An open that starts
 *    later stops there as well; before the fault it only reads.)
 * 5. The files change, and the fault is lifted. The next page load opens the
 *    archive as a start would (from the keychain), damage and all.
 *
 * Also: the repair dialog and the backend's view of the same state, for the
 * specs to check the screens against. Synthetic data only.
 */
import {
  closeSync,
  copyFileSync,
  fstatSync,
  mkdtempSync,
  openSync,
  readSync,
  rmSync,
  writeFileSync,
  writeSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { expect, type Page } from '@playwright/test'
import { killAndRestartApp, unlockIfLocked } from './app'
import { backend, fixture, injectFault } from './fixture'

/** The archive's password since move.spec; restore and free-space kept it. */
export const ARCHIVE_PASSWORD = 'moving day, 2026'

/** The two files that hold saved conversations, under the data folder. */
export const conversationFile = {
  evidence: () =>
    join(fixture().dirs.projectRoot, 'archive', 'agent-history.db'),
  readModel: () =>
    join(fixture().dirs.projectRoot, 'derived', 'agent-history-read-model.db'),
}

/** Stops the app with the archive closed, runs `damage` on its files, and lets it open again. */
export async function withArchiveStopped(page: Page, damage: () => void) {
  await page.goto('about:blank')
  const lift = injectFault('browsing-group-open')
  try {
    await backend('clear_session_database_key')
    await killAndRestartApp()
    // Waits for the new app's open at start, which the fault ends. An
    // unencrypted archive has no key in the keychain to try.
    await expect(backend('unlock_archive_from_keychain')).rejects.toThrow(
      /browsing-group-open|keychain-key-missing/,
    )
    damage()
  } finally {
    lift()
  }
}

/** Replaces a database with bytes no engine reads, as the Rust lifecycle tests do. */
export function overwriteWhole(file: string, bytes: string) {
  writeFileSync(file, bytes)
}

/** The page size a SQLite-format file keeps at offset 16 (1 means 65 536). */
function pageSize(fd: number) {
  const header = Buffer.alloc(2)
  readSync(fd, header, 0, 2, 16)
  const size = header.readUInt16BE(0)
  return size === 1 ? 65_536 : size
}

/**
 * The root page of `table` in an unencrypted database, read from a private
 * copy so nothing touches the live file. The live archive must be closed and
 * unencrypted.
 */
export function rootPageOf(file: string, table: string) {
  const dir = mkdtempSync(join(tmpdir(), 'pathkeep-root-page-'))
  try {
    const copy = join(dir, 'copy.db')
    copyFileSync(file, copy)
    const db = new DatabaseSync(copy)
    try {
      const row = db
        .prepare('SELECT rootpage FROM sqlite_schema WHERE name = ?')
        .get(table) as { rootpage: number } | undefined
      if (!row || row.rootpage < 2)
        throw new Error(`${table} has no root page of its own in ${file}`)
      return row.rootpage
    } finally {
      db.close()
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

/**
 * Fills one page with 0x55, keeping page 1's 100-byte header, as the Rust
 * lifecycle tests do (`damage/files.rs`). The file keeps its length.
 */
export function overwritePage(file: string, page: number) {
  const fd = openSync(file, 'r+')
  try {
    const size = pageSize(fd)
    const length = fstatSync(fd).size
    if (size < 512 || page * size > length)
      throw new Error(
        `page ${page} of ${size} bytes is past the end of ${file}`,
      )
    const skip = page === 1 ? 100 : 0
    writeSync(
      fd,
      Buffer.alloc(size - skip, 0x55),
      0,
      size - skip,
      (page - 1) * size + skip,
    )
    const check = Buffer.alloc(4)
    readSync(fd, check, 0, 4, (page - 1) * size + skip)
    if (check.readUInt32BE(0) !== 0x55555555)
      throw new Error(`page ${page} of ${file} was not written`)
  } finally {
    closeSync(fd)
  }
}

interface KindRow {
  kind: string
  state: string
  availability: string
  failure?: { role: string; stage: string; reason: string }
}

/** Agent conversations' row as `list_data_kinds` answers now. */
export async function agentKind() {
  const list = await backend<{ kinds: KindRow[] }>('list_data_kinds', {
    request: {},
  })
  const row = list.kinds.find((item) => item.kind === 'agent-history')
  if (!row) throw new Error('list_data_kinds has no agent-history row')
  return row
}

export interface Copy {
  path: string
  createdAt: string
  sourceOp: string
  verifiedOpenable: boolean
  unchecked: boolean
  conversationsUnreadable: boolean
  sizeBytes: number
}

export const recoveryCopies = () => backend<Copy[]>('list_recovery_snapshots')

/** Opens the Agent backup page after a restart: App Lock, then the archive from the keychain. */
export async function openAgentBackup(page: Page) {
  await page.goto('/#/agent/backup')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Backup', level: 1 }),
  ).toBeVisible({ timeout: 60_000 })
}

/** The damage notice, wherever it is shown. */
export const damageNotice = (page: Page) =>
  page.locator('[data-store-damage="agent-history"]')

export const repairDialog = (page: Page) =>
  page.getByRole('dialog', { name: 'Repair Agent conversations' })

/** The plan the dialog shows for a check that finished. */
export const repairPlan = (page: Page) =>
  repairDialog(page).locator('[data-repair-action]')

/** Whether `password` opens the archive, asked without changing anything. */
export async function opensArchive(password: string) {
  try {
    await backend('preview_rekey_archive', {
      request: {
        newMode: 'Encrypted',
        newKey: 'unused new password',
        currentKey: password,
      },
    })
    return true
  } catch (error) {
    expect(String(error)).toContain('archive-password-wrong')
    return false
  }
}

/** Changes the archive's password (or turns encryption on or off) as Settings → Security would, keychain included. */
export async function rekey(request: {
  newMode: 'Encrypted' | 'Plaintext'
  newKey: string | null
  currentKey?: string
}) {
  await backend('rekey_archive', {
    request: {
      ...request,
      rememberInKeychain: request.newMode === 'Encrypted',
    },
  })
}
