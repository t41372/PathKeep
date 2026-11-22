/**
 * The data folder as the archive-protection specs see it: its settings file
 * (`config.json`), a fingerprint of every file in it to prove a screen and
 * a start changed nothing, the startup decision (`get_archive_access`), and the
 * recovery screen and lock screens on the way into the app.
 *
 * These specs run at the end of the chain, after upgrade-v05.spec: the
 * archive there came from PathKeep 0.5.0's move file, with App Lock
 * passcode 4826.
 */
import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, type Page } from '@playwright/test'
import type { ArchiveAccess } from '../../../src/lib/backend-client/archive-access'
import { backend, fixture } from './fixture'

/** The App Lock passcode the archive has carried since lock.spec. */
export const PASSCODE = '4826'

/** move.spec's file and the password it was made with. */
export const moveFile = () =>
  path.join(fixture().root, 'move', 'pathkeep-move.pathkeep')
export const MOVE_FILE_PASSWORD = 'moving day, 2026'

/** PathKeep 0.5.0's own data, made by that release (tests/fixtures/v0.5-archive). */
export const V05_FIXTURE = path.resolve('tests/fixtures/v0.5-archive')

export interface V05Expected {
  password: string
  exportPassword: string
  appLockPasscode: string
  totalVisits: number
  /** The safety copy 0.5.0 made when its password was changed. */
  safetyCopy: { name: string; sourceOp: string }
}

export const v05Expected = () =>
  JSON.parse(
    readFileSync(path.join(V05_FIXTURE, 'expected.json'), 'utf8'),
  ) as V05Expected

export const dataRoot = () => fixture().dirs.projectRoot
export const controlPath = () => path.join(dataRoot(), 'config.json')

export interface Control {
  initialized?: boolean
  /** `"Encrypted"` as PathKeep 0.5 writes it, or this version's stamped form. */
  archiveMode:
    | string
    | {
        mode: string
        compatibility: {
          revision: number
          minReader: number
          minWriter: number
        }
      }
}

export function readControl() {
  return JSON.parse(readFileSync(controlPath(), 'utf8')) as Control
}

export function writeControl(control: unknown) {
  writeFileSync(controlPath(), JSON.stringify(control, null, 2))
}

/**
 * What a start writes whatever happens, and nothing else does: the logs, the
 * crash reports, and the owner lock, which holds the running app's process id.
 */
const WRITTEN_BY_EVERY_START = [
  'logs',
  'diagnostics',
  path.join('archive', '.pk-owner.lock'),
]

/**
 * Every file under the data folder with a hash of its bytes: if anything
 * writes, renames, adds or removes a file of the archive, the fingerprint
 * changes. What every start writes is left out.
 */
export function folderFingerprint(dir = dataRoot()) {
  const lines: string[] = []
  const walk = (folder: string) => {
    for (const entry of readdirSync(folder, { withFileTypes: true })) {
      const full = path.join(folder, entry.name)
      const relative = path.relative(dir, full)
      if (WRITTEN_BY_EVERY_START.includes(relative)) continue
      if (entry.isDirectory()) walk(full)
      else {
        const hash = createHash('sha256').update(readFileSync(full))
        lines.push(`${relative} ${hash.digest('hex')}`)
      }
    }
  }
  walk(dir)
  return lines.sort()
}

export function archiveAccess() {
  return backend<ArchiveAccess>('get_archive_access', {})
}

/** The recovery screen, whatever it says. */
export const accessScreen = (page: Page) =>
  page.locator('[data-archive-access]')

/**
 * App Lock's passcode, then the archive's password, whichever the app asks
 * for, until `settled` shows.
 */
export async function enterApp(
  page: Page,
  archivePassword: string,
  settled = page.getByRole('navigation', { name: 'Main navigation' }),
) {
  const appLock = page.getByRole('heading', { name: 'PathKeep is locked' })
  const archiveLock = page.getByRole('heading', {
    name: 'Your archive is encrypted',
  })
  for (let step = 0; step < 3; step++) {
    await expect(appLock.or(archiveLock).or(settled).first()).toBeVisible({
      timeout: 300_000,
    })
    if (await appLock.isVisible()) {
      await page.getByPlaceholder('Passcode').fill(PASSCODE)
      await page.getByPlaceholder('Passcode').press('Enter')
      await expect(appLock).toBeHidden()
    } else if (await archiveLock.isVisible()) {
      await page.getByPlaceholder('Password').fill(archivePassword)
      await page.getByPlaceholder('Password').press('Enter')
      await expect(archiveLock).toBeHidden({ timeout: 300_000 })
    } else return
  }
  await expect(settled).toBeVisible()
}

/**
 * `url` loaded from scratch. Going to an address that differs only after the
 * `#` doesn't load the page again, and a page loaded before a restart still
 * shows the old app until it is.
 */
export async function loadFresh(page: Page, url: string) {
  await page.goto(url)
  await page.reload()
}

/** The recovery screen, after App Lock if the app asks for it first. */
export async function openAccessScreen(page: Page) {
  await loadFresh(page, '/')
  const appLock = page.getByRole('heading', { name: 'PathKeep is locked' })
  await expect(appLock.or(accessScreen(page)).first()).toBeVisible({
    timeout: 60_000,
  })
  if (await appLock.isVisible()) {
    await page.getByPlaceholder('Passcode').fill(PASSCODE)
    await page.getByPlaceholder('Passcode').press('Enter')
  }
  await expect(accessScreen(page)).toBeVisible()
  return accessScreen(page)
}

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
  } catch {
    return false
  }
}

export interface ConversationImport {
  id: string
  importedAt: string
  added: number
  eligible: number
  previousCopy: string | null
}

export function conversationImports() {
  return backend<ConversationImport[]>('list_conversation_imports', {})
}

export interface Copy {
  id: string
  path: string
  sourceOp: string
  dataKinds: string[]
  verifiedOpenable: boolean
}

export function safetyCopies() {
  return backend<Copy[]>('list_recovery_snapshots')
}

/** The Storage row titled `title`: its buttons, scoped so no other row's is taken. */
export const storageRow = (page: Page, title: string) =>
  page.getByText(title, { exact: true }).locator('xpath=../..')
