/**
 * A pi session in a format version PathKeep cannot read yet
 * (`agent-history.md` §6): a newer pi wrote it (the fixture's
 * `addAgentConversation` with `formatVersion`). Pi is switched on from the
 * backup page's sources ("Back up Pi": look at its folder, then add it).
 *
 * Proves, with a notice alone:
 * - the backup that meets the version succeeds: nothing went unsaved, and
 *   it raises one notice, for Pi and that version;
 * - that notice is one more item needing a look (`get_task_summary`, the
 *   rail), listed on the background tasks page with the sentence that the
 *   files are saved as they are and will be readable after an update, and
 *   only the notice's own Dismiss (`dismiss-notice`); the backup page shows
 *   the same notice, and the run's details list it and note the file in
 *   that format version, as new;
 * - that Dismiss clears it everywhere: the item, the count, the backup
 *   page's notice, the run's pending notice;
 * - the next backup reads the same file again and brings nothing back: no
 *   notice, no item needing a look, the file's note listed but not new.
 *
 * And with a notice and a save failure in one backup (a newer version 1001,
 * and Codex's folder unreadable; POSIX only):
 * - the backup is partial and offers Try again, Dismiss and the notice's
 *   own Dismiss;
 * - dismissing the notice on the backup page leaves the save failure under
 *   Needs a look, with Try again and Dismiss (the bug this fixes: one
 *   Dismiss used to put both away);
 * - the run's details, from the run's own task, still offer Dismiss, which
 *   then puts the save failure away.
 *
 * Pi is switched off again at the end, so the specs after this one back up
 * the tools they expect. agent-backup-restart.spec checks the dismissals
 * hold across a restart.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  addAgentConversation,
  setAgentToolReadable,
} from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  agentRunWhole,
  agentRuns,
  agentSources,
  agentTasks,
  count,
  idleIndicatorName,
  newestRunRef,
  noteIsAbout,
  openBackupPage,
  railAttention,
  sourceNotes,
  sourceRow,
  taskIndicator,
  waitForRunAfter,
} from './support/agent-backup'
import { agentSettings } from './support/agent-tools'
import { unlockIfLocked } from './support/app'
import { backend, fixture, replaceAgents } from './support/fixture'

const VERSION = 1000
const NOTICE =
  'PathKeep can’t read Pi format 1000 yet. Its files are saved as they are and will be readable after an update.'
const DISMISS_NOTICE = (version: number) =>
  `Dismiss the notice about Pi format ${version}`
const DISMISS_TASK = 'Dismiss “Back up Agent conversations” from Needs a look'

async function attentionCount() {
  const summary = await backend<{ attentionCount: string }>(
    'get_task_summary',
    { request: {} },
  )
  return count(summary.attentionCount)
}

/** "Back up now" on the backup page, to its end. */
async function backUpNow(page: Page) {
  const previous = await newestRunRef()
  await page.getByRole('button', { name: 'Back up now' }).click()
  return waitForRunAfter(previous)
}

/** Setting up for the specs after this one, not what is tested. */
async function switchPiOff() {
  const now = await agentSettings()
  if (!now.policy.enabledTools.includes('pi')) return
  await backend('update_agent_sources', {
    request: { expectedToken: now.policyToken, toolChanges: { pi: false } },
  })
}

test.afterAll(async () => {
  await switchPiOff()
})

test('a format version PathKeep can’t read yet asks for a look once, and Dismiss keeps it away', async ({
  page,
}, testInfo) => {
  const attentionBefore = (await agentTasks('attention')).length
  const countBefore = await attentionCount()
  const runsBefore = (await agentRuns()).length

  const agents = addAgentConversation(fixture().agents, 'pi', {
    formatVersion: VERSION,
  })
  replaceAgents(agents)
  const written = agents.conversations.find((c) => c.formatVersion === VERSION)!
  expect(written.files).toHaveLength(1)

  // Pi is switched on: its folder is looked at first, then added.
  await openBackupPage(page)
  await sourceRow(page, 'pi')
    .getByRole('switch', { name: 'Back up Pi' })
    .click()
  const look = page.getByRole('dialog', { name: 'Back up Pi' })
  await expect(look).toBeVisible()
  await look
    .getByRole('button', { name: 'Add', exact: true })
    .click({ timeout: 60_000 })
  await expect(look).toBeHidden()
  await expect
    .poll(async () => (await agentSettings()).policy.enabledTools)
    .toContain('pi')

  // The backup that meets the version: saved, and one notice.
  const run = await backUpNow(page)
  const detail = await agentRunWhole(run.runRef)
  expect(detail.warnings).toEqual([])
  expect(run.result).toBe('success')
  expect(detail.formatNotices).toEqual([
    { tool: 'pi', version: String(VERSION) },
  ])
  const noted = detail.notes.filter((note) =>
    noteIsAbout(note, written.files[0]),
  )
  expect(
    noted.map((note) => [note.code, note.detail, note.isNew]),
  ).toContainEqual(['pi-unsupported-version', String(VERSION), true])

  // One more needs a look, and it offers only the notice's Dismiss.
  const waiting = await agentTasks('attention')
  expect(waiting).toHaveLength(attentionBefore + 1)
  const task = waiting.find((item) => item.runRef === run.runRef)
  expect(task).toMatchObject({
    state: 'succeeded',
    attention: true,
    actions: ['dismiss-notice'],
    formatNotices: [{ tool: 'pi', version: String(VERSION) }],
  })
  expect(await attentionCount()).toBe(countBefore + 1)
  await expect(taskIndicator(page)).toHaveAccessibleName(
    idleIndicatorName(await railAttention()),
  )

  // The backup page: the notice, and Pi kept as it is, not a problem.
  const card = page.locator('[data-format-notice-task]')
  await expect(card).toHaveCount(1)
  await expect(card).toContainText(NOTICE)
  await expect(
    card.getByRole('button', { name: DISMISS_NOTICE(VERSION) }),
  ).toBeVisible()
  const pi = sourceRow(page, 'pi')
  await expect(pi).not.toContainText('Needs a look')
  expect(sourceNotes(await agentSources(), 'pi').codes).toContain(
    'pi-unsupported-version',
  )
  await testInfo.attach('the backup page with the notice', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // The run's details: the notice, and the file's note in plain words.
  await page.getByRole('button', { name: /^Recent backups/ }).click()
  await page
    .locator(`li[data-run-ref="${run.runRef}"]`)
    .getByRole('button', { name: /^Details of the backup at / })
    .click()
  const sheet = page.getByRole('dialog')
  await expect(
    sheet.getByRole('region', { name: 'Formats PathKeep can’t read yet' }),
  ).toContainText(NOTICE)
  const note = sheet
    .getByRole('region', { name: 'Notes' })
    .locator('li[data-code="pi-unsupported-version"]')
  await expect(note).toContainText(
    'Pi: In format version 1000, which PathKeep can’t read yet; it will be readable after an update.',
  )
  await expect(note).toContainText('New')
  await testInfo.attach('the run’s details', {
    body: await sheet.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')

  // Background tasks: listed under Needs a look, with the notice's Dismiss.
  await page.goto('/#/tasks')
  await unlockIfLocked(page)
  const row = page
    .locator('section[data-task-group="attention"]')
    .locator(`li[data-run-ref="${run.runRef}"]`)
  await expect(row).toContainText(NOTICE)
  await expect(row.getByRole('button')).toHaveCount(1)
  await expect(
    row.getByRole('button', { name: DISMISS_NOTICE(VERSION) }),
  ).toHaveText('Dismiss')
  await testInfo.attach('the notice on the background tasks page', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
  await row.getByRole('button', { name: DISMISS_NOTICE(VERSION) }).click()
  await expect(row).toHaveCount(0)
  expect(await agentTasks('attention')).toHaveLength(attentionBefore)
  expect(await attentionCount()).toBe(countBefore)
  expect((await agentRunWhole(run.runRef)).formatNotices ?? []).toEqual([])
  await openBackupPage(page)
  await expect(page.locator('[data-format-notice-task]')).toHaveCount(0)

  // The next backup reads the same file and brings nothing back.
  const next = await backUpNow(page)
  const nextDetail = await agentRunWhole(next.runRef)
  expect(nextDetail.warnings).toEqual([])
  expect(next.result).toBe('success')
  expect(nextDetail.formatNotices ?? []).toEqual([])
  expect(count(nextDetail.newNoteCount)).toBe(0)
  expect(
    nextDetail.notes
      .filter((item) => noteIsAbout(item, written.files[0]))
      .map((item) => [item.code, item.isNew]),
  ).toContainEqual(['pi-unsupported-version', false])
  expect(await agentTasks('attention')).toHaveLength(attentionBefore)
  expect(await attentionCount()).toBe(countBefore)
  await expect(page.locator('[data-format-notice-task]')).toHaveCount(0)
  await expect(taskIndicator(page)).toHaveAccessibleName(
    idleIndicatorName(await railAttention()),
  )
  expect(await agentRuns()).toHaveLength(runsBefore + 2)
  await testInfo.attach('the backup page after the next backup', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})

test('dismissing a notice leaves what was not saved under Needs a look', async ({
  page,
}, testInfo) => {
  test.skip(process.platform === 'win32', 'chmod does not stop reads there')
  const NEWER = 1001
  const attentionBefore = (await agentTasks('attention')).length
  const countBefore = await attentionCount()
  expect(
    (await agentSettings()).policy.enabledTools,
    'the test before switched Pi on',
  ).toContain('pi')

  let agents = addAgentConversation(fixture().agents, 'pi', {
    formatVersion: NEWER,
  })
  agents = setAgentToolReadable(agents, 'codex', false)
  replaceAgents(agents)
  try {
    await openBackupPage(page)
    const run = await backUpNow(page)
    const detail = await agentRunWhole(run.runRef)
    expect(run.result).toBe('partial')
    expect(count(detail.warningCount)).toBeGreaterThan(0)
    expect(detail.formatNotices).toEqual([
      { tool: 'pi', version: String(NEWER) },
    ])
    const waiting = await agentTasks('attention')
    expect(waiting).toHaveLength(attentionBefore + 1)
    const task = waiting.find((item) => item.runRef === run.runRef)!
    expect(task.state).toBe('partial')
    expect([...task.actions].sort()).toEqual(
      ['dismiss', 'dismiss-notice', 'retry'].sort(),
    )

    // The notice's own Dismiss on the backup page.
    const card = page.locator('[data-format-notice-task]')
    await expect(card).toHaveCount(1)
    await card.getByRole('button', { name: DISMISS_NOTICE(NEWER) }).click()
    await expect(card).toHaveCount(0)

    // What was not saved still needs a look.
    await expect
      .poll(async () =>
        (await agentTasks('attention')).find(
          (item) => item.runRef === run.runRef,
        ),
      )
      .toMatchObject({ state: 'partial', attention: true })
    const left = (await agentTasks('attention')).find(
      (item) => item.runRef === run.runRef,
    )!
    expect(left.formatNotices ?? []).toEqual([])
    expect([...left.actions].sort()).toEqual(['dismiss', 'retry'])
    expect(await attentionCount()).toBe(countBefore + 1)
    await expect(
      page.getByText('The last backup didn’t save everything.'),
    ).toBeVisible()
    await testInfo.attach('the notice dismissed, the failure kept', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // The run's details offer Dismiss from the run's own task.
    await page.getByRole('button', { name: 'See what happened' }).click()
    const sheet = page.getByRole('dialog')
    await expect(page).toHaveURL(/\?run=agent-history/)
    await expect(
      sheet.getByRole('region', { name: 'Formats PathKeep can’t read yet' }),
    ).toHaveCount(0)
    await expect(
      sheet.getByRole('button', {
        name: 'Try “Back up Agent conversations” again',
      }),
    ).toBeVisible()
    await sheet.getByRole('button', { name: DISMISS_TASK }).click()
    await expect(sheet.getByRole('button', { name: DISMISS_TASK })).toHaveCount(
      0,
    )
    await expect
      .poll(async () => (await agentTasks('attention')).length)
      .toBe(attentionBefore)
    expect(await attentionCount()).toBe(countBefore)
    await testInfo.attach('the run’s details after Dismiss', {
      body: await sheet.screenshot(),
      contentType: 'image/png',
    })
  } finally {
    replaceAgents(setAgentToolReadable(agents, 'codex', true))
  }
})
