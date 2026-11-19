/**
 * The file that holds the conversations' lists, search and insights (the
 * read model) is damaged while PathKeep is stopped; the saved conversations
 * themselves are intact (`agent-history.md` › 從安全副本修復對話).
 *
 * Fixture state: runs after free-space.spec (App Lock on, passcode 4826; the
 * archive encrypted with move.spec's password, remembered by the keychain),
 * with the Agent conversations the agent specs saved, carried through the
 * move. The spec reads what it needs from the backend first.
 *
 * Proves:
 * - reopened, browsing history is all there, and the Agent backup page and
 *   Manage data types say which file is damaged, that browsing is not
 *   affected, and offer the repair;
 * - the repair checks first and offers only the rebuild, with nothing to
 *   choose; the plan counts the conversations and names the safety copy that
 *   will hold what it replaces;
 * - after "Rebuild the lists" the result counts the same conversations and
 *   names the same copy; the rebuilt lists hold at least every conversation
 *   the saved files hold; the page shows its count again, without the
 *   notice, and the folder approval is unchanged;
 * - the kept copy is listed (made before a conversation repair, marked as a
 *   copy whose conversations couldn't be read), counted, and Free up space
 *   frees it on its own.
 */
import { expect, test } from '@playwright/test'
import {
  agentKindState,
  agentOverview,
  conversationsText,
  count,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import {
  agentKind,
  conversationFile,
  damageNotice,
  openAgentBackup,
  overwriteWhole,
  recoveryCopies,
  repairDialog,
  repairPlan,
  withArchiveStopped,
} from './support/conversation-damage'
import { openArchiveVisitTotal, parseCount } from './support/fixture'

test('lists whose file is damaged are rebuilt from the saved conversations', async ({
  page,
}, testInfo) => {
  // A restart, a rebuild and a copy of the archive, in a debug build.
  test.setTimeout(600_000)
  await openAgentBackup(page)
  expect((await agentKind()).availability).toBe('available')
  const before = count((await agentOverview()).roots)
  expect(before, 'conversations saved by the agent specs').toBeGreaterThan(0)
  const visits = await openArchiveVisitTotal()
  const copiesBefore = (await recoveryCopies()).map((copy) => copy.path)
  const approval = (await agentKindState())?.hasBackupSource

  await withArchiveStopped(page, () =>
    overwriteWhole(conversationFile.readModel(), 'damaged lists'),
  )

  // Reopened: browsing is all there; the conversations say which file.
  await openAgentBackup(page)
  expect(await openArchiveVisitTotal()).toBe(visits)
  const damaged = await agentKind()
  expect(damaged.availability).toBe('unavailable')
  expect(damaged.failure?.role).toBe('read-model')
  const notice = damageNotice(page)
  await expect(notice).toContainText(
    'The file that holds their lists, search and insights is damaged or can’t be read.',
  )
  await expect(notice).toContainText('Browsing history is not affected.')
  // Manage data types says the same, with the same way out.
  await page.goto('/#/settings/kinds')
  await expect(
    damageNotice(page).getByRole('button', { name: 'Repair conversations…' }),
  ).toBeVisible()
  await page.goto('/#/agent/backup')
  await testInfo.attach('the damage notice', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  await notice.getByRole('button', { name: 'Repair conversations…' }).click()
  const dialog = repairDialog(page)
  await expect(dialog).toBeVisible()
  expect(page.url()).toContain('repair=agent')
  await expect(dialog).toContainText(
    'The file that holds their lists, search and insights is damaged or can’t be read.',
  )
  const plan = repairPlan(page)
  await expect(plan).toHaveAttribute('data-repair-action', 'rebuild', {
    timeout: 180_000,
  })
  await expect(
    plan.getByRole('heading', {
      name: 'Rebuild the lists from the saved conversations',
    }),
  ).toBeVisible()
  // Intact conversations leave nothing to choose and no folders to approve again.
  await expect(dialog.getByRole('radiogroup')).toHaveCount(0)
  await expect(plan).not.toContainText('choose the folders')
  const planned = parseCount(
    await plan.getByText(/can be read again\.$/).innerText(),
  )
  // The rebuilt lists (subagents count as conversations there) hold every
  // conversation the saved files hold. Nothing on this branch reads the
  // lists back conversation by conversation yet; the count is what shows.
  expect(planned, 'conversations in the rebuilt lists').toBeGreaterThanOrEqual(
    before,
  )
  const keptAs = await plan.locator('[data-kept-as]').innerText()
  expect(keptAs).toContain('before-agent-repair-')
  expect(copiesBefore).not.toContain(keptAs)
  await testInfo.attach('the rebuild plan', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await dialog.getByRole('button', { name: 'Rebuild the lists' }).click()
  await expect(
    dialog.getByRole('heading', { name: 'Saved conversations open again' }),
  ).toBeVisible({ timeout: 300_000 })
  await expect(
    dialog.getByText(
      `${planned.toLocaleString('en-US')} conversation${planned === 1 ? '' : 's'} can be read.`,
    ),
  ).toBeVisible()
  await expect(dialog.locator('[data-kept-copy]')).toHaveText(keptAs)
  await expect(
    dialog.getByText('Before the next backup, choose the folders'),
  ).toHaveCount(0)
  await testInfo.attach('the result', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(dialog).toHaveCount(0)

  // The conversations are found again where they were, and nothing says damage.
  await expect(damageNotice(page)).toHaveCount(0)
  await expect(
    page.getByText(new RegExp(`· ${conversationsText(before)}\\.`)),
  ).toBeVisible()
  expect((await agentKind()).availability).toBe('available')
  expect(count((await agentOverview()).roots)).toBe(before)
  expect(
    (await agentKindState())?.hasBackupSource,
    'a rebuild keeps this computer’s folder approval',
  ).toBe(approval)

  // What the rebuild replaced: listed, marked, counted, and freed on its own.
  const kept = (await recoveryCopies()).find((copy) => copy.path === keptAs)
  expect(kept).toMatchObject({
    sourceOp: 'agentRepair',
    conversationsUnreadable: true,
    verifiedOpenable: true,
  })
  expect(kept!.sizeBytes).toBeGreaterThan(0)
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await page
    .getByText('Free up space', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Review…' })
    .click()
  const free = page.getByRole('dialog')
  const keptRow = free.getByRole('listitem').filter({
    has: page.getByRole('checkbox', {
      name: /^Before a conversation repair.*Conversations couldn’t be read/,
    }),
  })
  await expect(keptRow).toHaveCount(1)
  // Only that copy: leftovers are ticked by default.
  for (const ticked of await free
    .getByRole('checkbox', { checked: true })
    .all())
    await ticked.click()
  await keptRow.getByRole('checkbox').click()
  const size = await keptRow.locator('.font-mono').innerText()
  await free.getByRole('button', { name: `Delete ${size}` }).click()
  await expect(page.getByText(`Freed ${size}.`)).toBeVisible({
    timeout: 60_000,
  })
  expect((await recoveryCopies()).map((copy) => copy.path)).not.toContain(
    keptAs,
  )
  expect(await openArchiveVisitTotal()).toBe(visits)
})
