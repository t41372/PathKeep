/**
 * One tool's folder cannot be read when the backup runs (`data-spaces.md`
 * §4, §5.1): Codex's folder loses its permissions (`setAgentToolReadable`)
 * and gets them back before "Try again". POSIX only, like the mutator.
 *
 * What the fixture's two tools hold (an empty file, a cut-off line) leaves
 * notes, never a reason for a look; still, the counts here are relative to
 * the backup before this spec and to the attention before it, so an
 * earlier spec's leftovers do not decide this one.
 *
 * Proves:
 * - the run ends as not having saved everything; Claude Code was read as
 *   the backup before read it, and Codex was not read, because of its folder;
 * - Codex's source row says it needs a look and, unfolded, why; the run's
 *   details give the same reason for Codex and list what was not saved;
 * - the rail counts one more needing a look, and leads to Settings ›
 *   the background tasks page, where this run is listed under Needs a look;
 * - with the folder readable again, "Try again" there reads Codex again,
 *   and this run leaves Needs a look (a successful retry clears its
 *   original).
 */
import { expect, test } from '@playwright/test'
import { setAgentToolReadable } from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  agentRun,
  agentRuns,
  agentTasks,
  count,
  idleIndicatorName,
  needsLookText,
  openBackupPage,
  openTaskList,
  railAttention,
  sourceRow,
  taskIndicator,
  waitForRunAfter,
} from './support/agent-backup'
import { fixture } from './support/fixture'

const UNREADABLE =
  'PathKeep couldn’t read its folder: no permission, or the disk isn’t connected. That doesn’t mean its files are gone.'

test('a tool that can’t be read leaves a partial run that a retry finishes', async ({
  page,
}, testInfo) => {
  test.skip(process.platform === 'win32', 'chmod does not stop reads there')
  const [previous] = await agentRuns(1)
  const previousDetail = await agentRun(previous.runRef)
  const claudeBefore = previousDetail.perTool?.find(
    (item) => item.tool === 'claude-code',
  )?.availability
  const attentionBefore = (await agentTasks('attention')).length

  let agents = setAgentToolReadable(fixture().agents, 'codex', false)
  try {
    await openBackupPage(page)
    await page.getByRole('button', { name: 'Back up now' }).click()
    const run = await waitForRunAfter(previous.runRef)
    expect(run.result).toBe('partial')
    const detail = await agentRun(run.runRef)
    const tool = (id: string) =>
      detail.perTool?.find((item) => item.tool === id)?.availability
    expect(tool('claude-code')).toBe(claudeBefore)
    expect(tool('codex')).toBe('root-unreadable')

    // The page: the header names the problem; Codex's row says why.
    await expect(
      page.getByText('The last backup didn’t save everything.'),
    ).toBeVisible()
    const codex = sourceRow(page, 'codex')
    await expect(codex).toContainText('Needs a look')
    await codex.getByRole('button', { name: 'Codex: see what is kept' }).click()
    await expect(codex).toContainText(UNREADABLE)
    await testInfo.attach('Codex’s row after the partial run', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })
    await page.getByRole('button', { name: 'See what happened' }).click()
    const sheet = page.getByRole('dialog')
    await expect(sheet.locator('li[data-tool="codex"]')).toContainText(
      UNREADABLE,
    )
    expect(count(detail.warningCount)).toBeGreaterThan(0)
    await expect(
      sheet.getByRole('region', { name: 'Not saved' }).locator('li[data-code]'),
    ).not.toHaveCount(0)
    await testInfo.attach('the run’s details', {
      body: await sheet.screenshot(),
      contentType: 'image/png',
    })
    await page.keyboard.press('Escape')

    // The rail: one more needs a look, which leads to the background tasks page.
    const attention = attentionBefore + 1
    expect((await agentTasks('attention')).length).toBe(attention)
    const onRail = await railAttention()
    await expect(taskIndicator(page)).toHaveAccessibleName(
      idleIndicatorName(onRail),
    )
    const list = await openTaskList(page)
    await list.getByRole('button', { name: needsLookText(onRail) }).click()
    await expect(page).toHaveURL(/#\/tasks$/)
    const group = page.locator('section[data-task-group="attention"]')
    const task = group.locator(`li[data-run-ref="${run.runRef}"]`)
    await expect(task).toBeVisible()
    await expect(task).toContainText('Didn’t save everything')
    await testInfo.attach('Needs a look on the background tasks page', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // Readable again: "Try again" reads Codex, and this run leaves Needs a look.
    agents = setAgentToolReadable(agents, 'codex', true)
    await task
      .getByRole('button', { name: 'Try “Back up Agent conversations” again' })
      .click()
    const retried = await waitForRunAfter(run.runRef)
    expect(retried.trigger).toBe('retry')
    expect(retried.result).toBe(previous.result)
    const retriedDetail = await agentRun(retried.runRef)
    expect(
      retriedDetail.perTool?.find((item) => item.tool === 'codex')
        ?.availability,
    ).not.toBe('root-unreadable')
    await expect(task).toHaveCount(0)
    const left = (await agentTasks('attention')).filter(
      (item) => item.runRef === run.runRef,
    )
    expect(left).toEqual([])
    await expect(taskIndicator(page)).toHaveAccessibleName(
      idleIndicatorName(await railAttention()),
    )
    await testInfo.attach('after the retry', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })
  } finally {
    setAgentToolReadable(agents, 'codex', true)
  }
})
