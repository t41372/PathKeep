/**
 * A tool's folder deleted and made again at the same path (`agent-history.md`
 * §6): the same path, a different folder, which PathKeep reads only once it
 * is approved again. Codex's folder is moved aside and copied back, so it
 * holds the same files in a new folder.
 *
 * A backup before the folder is replaced is the baseline the last backup
 * is compared with, so an earlier spec's leftovers do not decide this one.
 *
 * Proves:
 * - the backup ends as not having saved everything; Codex's folder is
 *   `root-changed`; its source row says why and offers "Look at it again…";
 * - "Try again" on that backup in Settings › Background work, which the
 *   backend answers with `previewRequired`, opens the look at that folder
 *   on the backup page (`?renew=`) instead of a dead end;
 * - the look says it is a different folder; "Approve and back up" approves
 *   it (its `directoryToken` changes) and starts a backup that reads it
 *   and ends as the baseline did;
 * - the old backup still needs a look (it cannot be retried as it was), and
 *   Dismiss puts it away.
 */
import { cpSync, existsSync, renameSync, rmSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import {
  agentRunWhole,
  agentSources,
  agentTasks,
  newestRunRef,
  openBackupPage,
  sourceRow,
  waitForRunAfter,
  type WireRunDetail,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { fixture } from './support/fixture'

const REPLACED =
  'Its folder was replaced by a different one since it was approved, so PathKeep didn’t read it. Look at it again before backing it up.'

const codexToken = async () =>
  (await agentSources()).find(
    (source) => source.tool === 'codex' && source.rootId,
  )?.directoryToken

/** How the run read Codex, by tool or by its folder (whichever it lists). */
const codexInRun = (detail: WireRunDetail) => [
  detail.perTool?.find((item) => item.tool === 'codex')?.availability,
  detail.roots?.find((item) => item.tool === 'codex')?.availability,
]

test('a folder made again at the same path is approved again, and backed up', async ({
  page,
}, testInfo) => {
  const root = fixture().agents.roots.codex
  const aside = `${root}.before-replace`
  const attentionBefore = (await agentTasks('attention')).length
  const tokenBefore = await codexToken()
  expect(tokenBefore, 'the approved Codex folder has a token').toBeDefined()

  await openBackupPage(page)
  const before = await newestRunRef()
  await page.getByRole('button', { name: 'Back up now' }).click()
  const baseline = await waitForRunAfter(before)

  renameSync(root, aside)
  cpSync(aside, root, {
    recursive: true,
    preserveTimestamps: true,
    verbatimSymlinks: true,
  })
  let approved = false
  try {
    await page.getByRole('button', { name: 'Back up now' }).click()
    const run = await waitForRunAfter(baseline.runRef)
    expect(run.result).toBe('partial')
    expect(codexInRun(await agentRunWhole(run.runRef))).toContain(
      'root-changed',
    )

    // Its row says why, and offers to look again.
    const codex = sourceRow(page, 'codex')
    await expect(codex).toContainText('Needs a look')
    await codex.getByRole('button', { name: 'Codex: see what is kept' }).click()
    await expect(codex).toContainText(REPLACED)
    await expect(
      codex.getByRole('button', { name: 'Look at it again…' }),
    ).toBeVisible()
    await testInfo.attach('Codex’s row with its folder replaced', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // "Try again" cannot run it as it was: it opens the look instead.
    await page.goto('/#/settings/background?show=conversations')
    await unlockIfLocked(page)
    const task = page
      .locator('section[data-task-group="attention"]')
      .locator(`li[data-run-ref="${run.runRef}"]`)
    await task
      .getByRole('button', { name: 'Try “Back up Agent conversations” again' })
      .click()
    await expect(page).toHaveURL(/#\/agent\/backup\?renew=/)
    const look = page.getByRole('dialog', {
      name: 'Look at Codex’s folder again',
    })
    await expect(look).toBeVisible()
    await expect(
      look.getByText(
        'This is a different folder from the one you approved. Approve it to back it up from now on.',
      ),
    ).toBeVisible({ timeout: 60_000 })
    await testInfo.attach('the look at the replaced folder', {
      body: await look.screenshot(),
      contentType: 'image/png',
    })
    const beforeNew = await newestRunRef()
    await look.getByRole('button', { name: 'Approve and back up' }).click()
    await expect(look).toBeHidden()
    approved = true
    await expect(page).not.toHaveURL(/renew=/)

    // Approved again, and backed up.
    await expect.poll(codexToken).not.toBe(tokenBefore)
    const fresh = await waitForRunAfter(beforeNew)
    expect(fresh.trigger).toBe('manual')
    expect(fresh.result).toBe(baseline.result)
    expect(codexInRun(await agentRunWhole(fresh.runRef))).not.toContain(
      'root-changed',
    )
    await expect(sourceRow(page, 'codex')).not.toContainText('Needs a look')

    // The old attempt still needs a look; Dismiss puts it away.
    await page.goto('/#/settings/background?show=conversations')
    await unlockIfLocked(page)
    await expect(task).toBeVisible()
    await task
      .getByRole('button', {
        name: 'Dismiss “Back up Agent conversations” from Needs a look',
      })
      .click()
    await expect(task).toHaveCount(0)
    expect(await agentTasks('attention')).toHaveLength(attentionBefore)
  } finally {
    if (approved) {
      rmSync(aside, { recursive: true, force: true })
    } else if (existsSync(aside)) {
      // Put the approved folder back for the specs after this one.
      rmSync(root, { recursive: true, force: true })
      renameSync(aside, root)
    }
  }
})
