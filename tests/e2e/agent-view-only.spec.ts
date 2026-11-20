/**
 * Conversations that came with the archive, on a computer that never chose
 * whether to back them up (`data-spaces.md` §2): they can be read and
 * searched without turning backup on, and nothing here is backed up.
 *
 * Proves:
 * - a browsing-only first run leaves Agent conversations off, with no
 *   choice made, so the backend says a moved file's conversations would be
 *   paused here, and the import check says it in words before anything
 *   changes: they come along, readable, not backed up;
 * - after opening move.spec's file, Agent conversations are paused with no
 *   backup source: the switcher lists the Agent space with the moved
 *   conversations, History lists them, and a search finds the one
 *   conversation that has the words;
 * - Manage data types says this computer isn't backing them up yet, with
 *   "Set up backup…"; the Agent backup page says nothing here is backed up
 *   and has no "Back up now".
 *
 * Runs after `onboarding-agent` on the empty data folder it leaves, opens
 * the file move.spec made (the archive as the projects up to `move` left
 * it, Agent conversations included), and deletes all data at the end the
 * way a user does, so `upgrade` finds the folder empty again.
 */
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Locator, type Page } from '@playwright/test'
import type { ImportPreview } from '../../src/lib/backend-client/migration'
import { agentKindState } from './support/agent-backup'
import {
  agentChat,
  listRows,
  openHistory,
  savedChat,
} from './support/agent-chats'
import { backend, fixture } from './support/fixture'

/** move.spec's file and the password it was made with. */
const moveFile = () => join(fixture().root, 'move', 'pathkeep-move.pathkeep')
const FILE_PASSWORD = 'moving day, 2026'
const WELCOME = 'Keep what gets deleted automatically'
const VIEW_ONLY =
  'This computer isn’t backing conversations up yet. The archive holds conversations saved on another computer, and you can read and search them.'

/** A first run that keeps browsing history only, the quickest way through. */
async function browsingOnlyFirstRun(page: Page) {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible({
    timeout: 60_000,
  })
  await page.getByRole('button', { name: 'Get started' }).click()
  await expect(
    page.getByRole('heading', { name: 'What should PathKeep keep?' }),
  ).toBeVisible()
  const agentKind = page.getByRole('checkbox', {
    name: 'Agent conversations',
    exact: true,
  })
  await expect(agentKind).toBeChecked()
  await agentKind.click()
  await expect(agentKind).not.toBeChecked()
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'Here’s what PathKeep found' }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'Where the archive lives' }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'Encrypt the archive?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Don’t encrypt/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'How often should we back up?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Manual/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()
  await expect(
    page.getByRole('heading', { name: 'Turn on AI features?' }),
  ).toBeVisible()
  await page.getByRole('radio', { name: /^Not now/ }).click()
  await page.getByRole('button', { name: 'Continue' }).click()
  await page.getByRole('button', { name: 'Start first backup' }).click()
  await expect(page.getByText(/Done\. [\d,]+ visits? saved\./)).toBeVisible({
    timeout: 120_000,
  })
  await page.getByRole('button', { name: 'Open PathKeep' }).click()
}

/** Opens the moved archive if the window asks for its password. */
async function unlockMovedArchive(page: Page, settled: Locator) {
  const locked = page.getByRole('heading', {
    name: 'Your archive is encrypted',
  })
  await expect(locked.or(settled)).toBeVisible({ timeout: 60_000 })
  if (await locked.isVisible()) {
    await page.getByPlaceholder('Password').fill(FILE_PASSWORD)
    await page.getByPlaceholder('Password').press('Enter')
    await expect(locked).toBeHidden()
  }
}

test('conversations that came with the archive can be read here without turning backup on', async ({
  page,
}, testInfo) => {
  // A first backup, then the whole archive written back from the file.
  test.setTimeout(600_000)
  expect(existsSync(moveFile()), 'move.spec left its file').toBe(true)
  await browsingOnlyFirstRun(page)

  // No choice made about Agent conversations: off, and the moved file's
  // would be paused here.
  expect((await agentKindState())?.state).toBe('off')
  const preview = await backend<ImportPreview>('preview_app_data_import', {
    bundlePath: moveFile(),
  })
  expect(preview.agentConversations).toEqual({
    carried: true,
    stateIfPresent: 'paused',
  })

  // The check says it before anything changes.
  await page.goto('/#/settings/storage')
  await page.getByRole('button', { name: 'Import…' }).click()
  const importing = page.getByRole('dialog')
  await importing.getByLabel('File').fill(moveFile())
  await importing.getByRole('button', { name: 'Check file' }).click()
  await expect(importing).toContainText(
    /Agent conversations\s*Readable, not backed up/,
    { timeout: 60_000 },
  )
  await expect(importing).toContainText(
    'Any Agent conversations in the file come along. This computer hasn’t chosen to back any up, so you can read and search them in the Agent space, and nothing here is backed up until you set it up.',
  )
  await testInfo.attach('the import check', {
    body: await importing.screenshot(),
    contentType: 'image/png',
  })
  expect((await agentKindState())?.state, 'nothing changed yet').toBe('off')
  await importing.getByLabel('Password of this file').fill(FILE_PASSWORD)
  await importing.getByRole('button', { name: 'Replace my archive' }).click()
  await expect(page.getByText('Import finished.')).toBeVisible({
    timeout: 300_000,
  })

  // Paused, with nothing on this computer to back up.
  await page.goto('/#/settings/kinds')
  await unlockMovedArchive(
    page,
    page.getByRole('heading', { name: 'Manage data types', level: 2 }),
  )
  await expect
    .poll(async () => {
      const row = await agentKindState()
      return row && { state: row.state, hasBackupSource: row.hasBackupSource }
    })
    .toEqual({ state: 'paused', hasBackupSource: false })
  const held = Number((await agentKindState())!.archiveCount)
  expect(held, 'the file carried conversations').toBeGreaterThan(0)

  const card = page.getByRole('region', {
    name: 'Agent conversations',
    exact: true,
  })
  await expect(
    card
      .getByRole('radiogroup', { name: 'State of Agent conversations' })
      .getByRole('radio', { name: 'Paused' }),
  ).toBeChecked()
  await expect(card).toContainText(VIEW_ONLY)
  await expect(
    card.getByRole('button', { name: 'Set up backup…' }),
  ).toBeVisible()

  // Paused is still a space: the switcher lists it.
  await page.getByRole('button', { name: /^Switch data type\. Now: / }).click()
  await expect(
    page
      .getByRole('menu', { name: /^Switch data type/ })
      .getByRole('menuitemradio', { name: /Agent conversations/ }),
  ).toBeVisible()
  await page.keyboard.press('Escape')

  // History lists them, and search finds the one with these words.
  await openHistory(page)
  await expect(listRows(page).first()).toBeVisible({ timeout: 60_000 })
  const large = await savedChat(
    agentChat(fixture().agents.scenes.large.conversation),
  )
  await openHistory(
    page,
    `?q=${encodeURIComponent('/running run 1\\d\\d now/')}`,
  )
  const results = page
    .getByRole('feed', { name: 'Search results' })
    .locator('[data-result-id]')
  await expect(results).toHaveCount(1, { timeout: 60_000 })
  await expect(results.first()).toHaveAttribute('data-result-id', large.id)
  await testInfo.attach('a search on this computer', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // The backup page: nothing here is backed up, and nothing to start.
  await page.goto('/#/agent/backup')
  await expect(
    page.getByText('This computer isn’t backing conversations up yet', {
      exact: true,
    }),
  ).toBeVisible()
  await expect(
    page.getByText(
      'Nothing on this computer is backed up. Set up backup to choose which tools to keep.',
    ),
  ).toBeVisible()
  await expect(page.getByRole('button', { name: 'Back up now' })).toHaveCount(0)
  expect((await agentKindState())?.state, 'still paused').toBe('paused')

  // Leave the data folder empty, the way a user deletes everything.
  await page.goto('/#/settings/storage')
  await page.getByRole('button', { name: 'Delete…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
  await dialog.getByRole('textbox').fill('DELETE')
  await dialog.getByRole('button', { name: 'Delete forever' }).click()
  await expect(page.getByRole('heading', { name: WELCOME })).toBeVisible({
    timeout: 60_000,
  })
  const after = await backend<{ config: { initialized: boolean } }>(
    'app_snapshot',
  )
  expect(after.config.initialized).toBe(false)
})
