/**
 * Settings → Background work and search ranking, on the archive the first
 * backup built.
 *
 * Proves: the Insights work the first backup queued shows up as done; a
 * paused queue really holds a new rebuild (it stays waiting), cancel and
 * retry change its state in the backend, and resuming runs it to done;
 * "Clear Insights data" previews the exact rows the backend will remove,
 * removes them, and a rebuild brings the same rows back; search ranking
 * values survive a reload because they are in the backend config.
 */
import { expect, test, type Locator, type Page } from '@playwright/test'
import type {
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  IntelligenceRuntimeSnapshot,
} from '../../src/lib/types'
import { backend, expectCount, parseCount } from './support/fixture'

const runtime = () =>
  backend<IntelligenceRuntimeSnapshot>('load_intelligence_runtime')
const clearPreview = () =>
  backend<ClearDerivedIntelligenceReport>('preview_clear_derived_intelligence')
const snapshot = () => backend<AppSnapshot>('app_snapshot')

/** The rebuild queued from the UI: the newest full rebuild in the backend. */
async function newestFullRebuild() {
  const { recentJobs } = await runtime()
  return recentJobs
    .filter((job) => job.jobType === 'full-rebuild')
    .sort((a, b) => b.id - a.id)[0]
}

function insightsRow(page: Page): Locator {
  return page
    .getByText('Insights and page details', { exact: true })
    .locator('xpath=ancestor::div[contains(@class,"rounded-xl")][1]')
}

function rebuildJob(page: Page): Locator {
  return insightsRow(page)
    .getByRole('listitem')
    .filter({ hasText: 'All browsers · Everything' })
    .first()
}

test('background work: queue, pause, cancel, retry, clear and rebuild', async ({
  page,
}, testInfo) => {
  await page.goto('/#/settings/background')
  await expect(
    page.getByRole('heading', { name: 'Background work' }),
  ).toBeVisible()

  // The first backup's Insights work is listed, and finished.
  const before = await runtime()
  expect(before.queue.succeeded).toBeGreaterThan(0)
  expect(before.queue.queued + before.queue.running).toBe(0)
  const row = insightsRow(page)
  await expect(row.getByText('Nothing running.')).toBeVisible()
  await expect(
    row.locator('li[data-job-state="succeeded"]').first(),
  ).toContainText('Done')
  await expect(row.locator('li[data-job-state="running"]')).toHaveCount(0)

  // Pause, then queue a rebuild: it waits instead of running.
  const run = page.getByRole('switch', { name: 'Run background work' })
  await run.click()
  await expect(run).not.toBeChecked()
  await expect
    .poll(async () => (await snapshot()).config.ai.jobQueuePaused)
    .toBe(true)
  await row.getByRole('button', { name: 'Rebuild' }).click()
  await page
    .getByRole('alertdialog', { name: 'Rebuild Insights?' })
    .getByRole('button', { name: 'Rebuild' })
    .click()
  const job = rebuildJob(page)
  await expect(job).toContainText('Waiting')
  const queued = await newestFullRebuild()
  expect(queued?.state).toBe('queued')
  // Still waiting a few seconds later: the pause holds it.
  await page.waitForTimeout(3_000)
  expect((await newestFullRebuild())?.state).toBe('queued')
  await expect(job).toContainText('Waiting')

  // Cancel it, then put it back in the queue.
  await job
    .getByRole('button', { name: 'Cancel: All browsers · Everything' })
    .click()
  await expect(job).toContainText('Cancelled')
  expect((await newestFullRebuild())?.state).toBe('cancelled')
  await job
    .getByRole('button', { name: 'Try again: All browsers · Everything' })
    .click()
  await expect(job).toContainText('Waiting')
  expect((await newestFullRebuild())?.state).toBe('queued')

  // Resume: the same job runs to done.
  await run.click()
  await expect(run).toBeChecked()
  await expect(job).toContainText('Done', { timeout: 60_000 })
  const finished = await newestFullRebuild()
  expect(finished?.id).toBe(queued?.id)
  expect(finished?.state).toBe('succeeded')

  // Clear Insights data: the preview shows what the backend will remove.
  const expected = await clearPreview()
  expect(expected.clearedVisitDerivedFactRows).toBeGreaterThan(0)
  await page.getByRole('button', { name: 'Clear…' }).click()
  const dialog = page.getByRole('alertdialog', {
    name: 'Clear Insights data?',
  })
  const removes = dialog.getByRole('list', { name: 'Removes:' })
  const visitsLine = removes.getByText(/^Details for [\d,]+ visits?$/)
  await expect(visitsLine).toBeVisible()
  await expectCount(
    testInfo,
    'visit details the clear preview lists',
    parseCount(await visitsLine.innerText()),
    expected.clearedVisitDerivedFactRows,
  )
  await expect(dialog.getByText(/Kept: your history/)).toBeVisible()
  await dialog.getByRole('button', { name: 'Clear', exact: true }).click()
  await expect(
    page.getByText('Insights data cleared. Insights are empty until rebuilt.'),
  ).toBeVisible()

  const afterClear = await clearPreview()
  expect(afterClear.clearedVisitDerivedFactRows).toBe(0)
  expect(afterClear.clearedStructuralRows).toBe(0)
  const modules = page.getByRole('list', { name: 'Insights data' })
  await expect(
    modules.locator('li[data-module="visit-derived-facts"]'),
  ).toContainText('Not built yet')

  // Rebuild from the toast: the same rows come back.
  await page.getByRole('button', { name: 'Rebuild now' }).click()
  await expect(
    modules.locator('li[data-module="visit-derived-facts"]'),
  ).toContainText('Up to date', { timeout: 60_000 })
  const rebuilt = await clearPreview()
  await expectCount(
    testInfo,
    'visit details after rebuilding',
    rebuilt.clearedVisitDerivedFactRows,
    expected.clearedVisitDerivedFactRows,
  )
})

test('search ranking values persist in the backend config', async ({
  page,
}) => {
  const initial = (await snapshot()).config.ai
  expect(initial.hybridRrfK).toBe(60)
  expect(initial.starredBoost).toBeCloseTo(0.15)

  await page.goto('/#/settings/ai')
  const k = page.getByLabel('Rank smoothing')
  const boost = page.getByLabel('Starred boost')
  const save = page.getByRole('button', { name: 'Save', exact: true })
  await expect(k).toHaveValue('60')

  // Out of range: flagged, and nothing can be saved.
  await boost.fill('0.9')
  await expect(page.getByText('Enter a number from 0 to 0.5.')).toBeVisible()
  await expect(save).toBeDisabled()

  await boost.fill('0.3')
  await k.fill('40')
  await save.click()
  await expect(page.getByText('Search ranking saved.')).toBeVisible()

  await page.reload()
  await expect(page.getByLabel('Rank smoothing')).toHaveValue('40')
  await expect(page.getByLabel('Starred boost')).toHaveValue('0.3')
  const saved = (await snapshot()).config.ai
  expect(saved.hybridRrfK).toBe(40)
  expect(saved.starredBoost).toBeCloseTo(0.3)
  expect(saved.lexicalWeight).toBe(1)

  await page.getByRole('button', { name: 'Reset to defaults' }).click()
  await expect(page.getByLabel('Rank smoothing')).toHaveValue('60')
  await expect
    .poll(async () => (await snapshot()).config.ai.hybridRrfK)
    .toBe(60)
  expect((await snapshot()).config.ai.starredBoost).toBeCloseTo(0.15)
})
