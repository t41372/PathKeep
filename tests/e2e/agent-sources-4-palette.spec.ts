/**
 * Agent conversations in ⌘K (`data-spaces.md` §6) and the Claude desktop
 * app's folder on the backup page, with both kinds on and Claude Code and
 * Codex backed up (what the specs before leave).
 *
 * Proves:
 * - "Back up now" is there once, for every kind that is on, and with two
 *   kinds on each also has its own; "Back up Agent conversations only"
 *   starts a conversation backup and nothing else;
 * - typing a tool's name finds its folders under "Agent conversations",
 *   which open the backup page; that group has no "Search … for …" row,
 *   since the backup page does not search, and words that match nothing
 *   of it leave no empty group behind;
 * - typing "backup at" finds recent conversation backups, which open their
 *   details;
 * - the Claude desktop app's folder says what it is, over its path.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  lastBrowsingRun,
  newestRunRef,
  openBackupPage,
  sourceRow,
  waitForRunAfter,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { fixture } from './support/fixture'

async function openPalette(page: Page, words: string) {
  await page.keyboard.press('ControlOrMeta+k')
  const palette = page.getByRole('dialog')
  await palette.getByRole('combobox').fill(words)
  return palette
}

test('⌘K backs up one kind or all, and finds Agent sources and backups', async ({
  page,
}, testInfo) => {
  await page.goto('/#/browsing/history')
  await unlockIfLocked(page)

  // One "Back up now" for everything, and one per kind.
  let palette = await openPalette(page, 'back up')
  const actions = palette.getByRole('group', { name: 'Actions' })
  await expect(
    actions.getByRole('option', { name: 'Back up now', exact: true }),
  ).toBeVisible()
  await expect(
    actions.getByRole('option', { name: 'Back up Browsing history only' }),
  ).toBeVisible()
  const agentOnly = actions.getByRole('option', {
    name: 'Back up Agent conversations only',
  })
  await expect(agentOnly).toBeVisible()
  await testInfo.attach('⌘K actions with two kinds on', {
    body: await palette.screenshot(),
    contentType: 'image/png',
  })
  const browsingBefore = await lastBrowsingRun()
  const previous = await newestRunRef()
  await agentOnly.click()
  const run = await waitForRunAfter(previous)
  expect(run.trigger).toBe('manual')
  expect((await lastBrowsingRun())?.id).toBe(browsingBefore?.id)

  // Words that match nothing of it: no group.
  palette = await openPalette(page, 'zzqx no such thing')
  await expect(
    palette.getByRole('group', { name: 'Agent conversations' }),
  ).toHaveCount(0)
  await page.keyboard.press('Escape')

  // A tool's folders.
  palette = await openPalette(page, 'codex')
  const results = palette.getByRole('group', { name: 'Agent conversations' })
  const codex = results.getByRole('option', { name: /^Codex/ })
  await expect(codex.first()).toBeVisible()
  await expect(
    results.getByRole('option', { name: /^Search “Agent conversations”/ }),
  ).toHaveCount(0)
  await codex.first().click()
  await expect(page).toHaveURL(/#\/agent\/backup$/)

  // Recent backups, which open their details.
  await page.goto('/#/settings/kinds')
  palette = await openPalette(page, 'backup at')
  const backup = palette
    .getByRole('group', { name: 'Agent conversations' })
    .getByRole('option', { name: /^Backup at / })
  await expect(backup.first()).toBeVisible()
  await backup.first().click()
  await expect(page).toHaveURL(/#\/agent\/backup\?run=agent-history/)
  await expect(page.getByRole('dialog')).toBeVisible()
  await page.keyboard.press('Escape')
})

test('the Claude desktop app’s folder says what it is', async ({ page }) => {
  const desktop = fixture().agents.roots['claude-desktop']
  await openBackupPage(page)
  const row = sourceRow(page, 'claude-code')
  await row
    .getByRole('button', { name: 'Claude Code: see what is kept' })
    .click()
  const folder = row.getByText(`Reads ${desktop}`, { exact: true })
  await expect(folder).toBeVisible()
  await expect(
    row.getByText('Claude desktop app (local Cowork sessions)', {
      exact: true,
    }),
  ).toBeVisible()
})
