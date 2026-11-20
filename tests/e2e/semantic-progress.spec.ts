/**
 * Responsibilities: prove progress and resume scope through normal backup/index controls.
 * Not responsible for mapping repair, snapshots, provider quality or unrelated queue races.
 * Standard browser fixtures supply title changes with new visits; internal archive data is untouched.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  appendChromeVisits,
  retitleChromePage,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import type { AiQueueStatus, BackupReport } from '../../src/lib/types'
import { addEmbeddingsService } from './support/ai-service'
import { startFakeEmbeddings } from './support/fake-embeddings'
import { backend, fixture } from './support/fixture'
import { progressScopeVisits } from './support/progress-scope'

const queue = () => backend<AiQueueStatus>('load_ai_queue_status')
async function idle() {
  await expect
    .poll(
      async () => {
        const q = await queue()
        return q.queued + q.running
      },
      { timeout: 240_000 },
    )
    .toBe(0)
}
async function backup(page: Page) {
  await page.goto('/#/backup')
  const button = page.getByRole('button', { name: 'Back up now' }).first()
  const completed = page.waitForResponse((response) =>
    response.url().endsWith('/commands/run_backup_now'),
  )
  await button.click()
  const response = await completed
  expect(response.ok()).toBe(true)
  const report = (await response.json()) as BackupReport
  await expect(button).toBeEnabled({ timeout: 120_000 })
  return report
}

test('updated visits and a later backup keep one truthful progress scope', async ({
  page,
}, info) => {
  test.setTimeout(600_000)
  const service = await startFakeEmbeddings()
  const before = await backend<{
    config: Record<string, unknown> & { ai: unknown }
  }>('app_snapshot')
  const observations: unknown[] = []
  try {
    const { original, updated, afterPause } = progressScopeVisits()
    const chrome = fixture().files['chrome:Default']
    appendChromeVisits(chrome, original)
    const firstBackup = await backup(page)
    expect(firstBackup.run?.newVisits).toBe(160)
    observations.push({
      checkpoint: 'original visits backed up',
      report: firstBackup,
    })
    await addEmbeddingsService(page, service.url, 'e2e-embed', 'Progress scope')
    const semantic = page.getByRole('switch', {
      name: 'Semantic search',
      exact: true,
    })
    if (await semantic.isChecked()) {
      await semantic.click()
      await expect(semantic).not.toBeChecked()
    }
    await semantic.click()
    // The build is admitted a moment after the click, so an idle queue right
    // after it proves nothing. Wait for the index itself, then for the queue.
    await expect
      .poll(async () => (await queue()).semanticIndex.chosenComplete, {
        timeout: 240_000,
      })
      .toBe(true)
    await idle()

    // Chrome stores one title per URL. The fixture's append helper alone preserves the old title.
    for (const visit of updated) {
      retitleChromePage(chrome, visit.url, visit.title)
    }
    appendChromeVisits(chrome, updated)
    service.holdAfter(1)
    const updateBackup = await backup(page)
    expect(updateBackup.run?.newVisits).toBe(4)
    observations.push({
      checkpoint: 'new observations backed up',
      report: updateBackup,
    })
    await expect
      .poll(() => service.held(), { timeout: 120_000 })
      .toBeGreaterThan(0)
    expect(
      service.inputs.some((text) => text.includes('Updated progress page')),
    ).toBe(true)
    await page.goto('/#/settings/ai')
    const during = (await queue()).semanticIndex.build!
    expect(during.progress.phase).toBe('visits')
    expect(during.total).not.toBeNull()
    expect(during.done).toBeGreaterThan(0)
    expect(during.done).toBeLessThan(during.total!)
    // The service holds its answer, so the count stands still: both screens
    // show that count and a bar at its share of the total, at once, without
    // counting up from zero.
    const number = new Intl.NumberFormat('en-US')
    const counts = `${number.format(during.done)} of ${number.format(during.total!)} visits`
    const percent = String(Math.floor((during.done / during.total!) * 100))
    const bar = page.getByRole('progressbar').first()
    await expect(bar).toHaveAttribute('aria-valuenow', percent)
    await expect(page.getByRole('main')).toContainText(counts)
    const label = await bar.getAttribute('aria-label')
    expect(label).toBeTruthy()
    observations.push({
      checkpoint: 'updated batch',
      queue: await queue(),
      label,
      value: Number(percent),
    })
    await page.screenshot({
      path: info.outputPath('updated-progress.png'),
      fullPage: true,
    })
    await page.goto('/#/tasks')
    const job = page.locator(`[data-job="ai-${during.jobId}"]`)
    await expect(job).toContainText(label!)
    await expect(job).toContainText(counts)
    await expect(job.getByRole('progressbar')).toHaveAttribute(
      'aria-label',
      label!,
    )
    await expect(job.getByRole('progressbar')).toHaveAttribute(
      'aria-valuenow',
      percent,
    )

    await page.goto('/#/settings/ai')
    await page.getByRole('button', { name: 'Pause', exact: true }).click()
    await expect
      .poll(async () => (await queue()).semanticIndex.build?.state)
      .toBe('paused')
    const frozenTotal = (await queue()).semanticIndex.build!.total
    const lateTitle = afterPause[0].title
    appendChromeVisits(chrome, afterPause)
    await backup(page)
    expect((await queue()).semanticIndex.build!.total).toBe(frozenTotal)
    service.release()
    await page.goto('/#/settings/ai')
    await page.getByRole('button', { name: 'Continue', exact: true }).click()
    // The same holds here: the queue can read idle before the paused job is
    // back in it. Wait for that job to finish; the update it admits on
    // finishing is in the queue by then, and `idle` waits for it.
    await expect
      .poll(
        async () =>
          (await queue()).recentJobs.find((job) => job.id === during.jobId)
            ?.state,
        { timeout: 240_000 },
      )
      .toBe('succeeded')
    await idle()
    const complete = await queue()
    // The job kept the scope it started with: its own counts, and no more.
    const finished = complete.recentJobs.find((job) => job.id === during.jobId)!
    expect(finished.summary).toContain('4 new / 160 updated')
    observations.push({ checkpoint: 'same scope completed', queue: complete })
    // The visit backed up during the pause was outside that scope. With
    // “index new visits after each backup” on, its update is admitted when
    // the job finishes, without another backup, and the work stops there.
    const later = complete.recentJobs.filter(
      (job) => job.jobType === 'index-build' && job.id > during.jobId,
    )
    expect(
      later.map((job) => job.state),
      'one more index job, finished',
    ).toEqual(['succeeded'])
    expect(service.inputs.some((text) => text.includes(lateTitle))).toBe(true)
    const status = (
      await backend<{ aiStatus: { state: string } }>('app_snapshot')
    ).aiStatus
    expect(status.state, 'the index is current').toBe('ready')
    observations.push({
      checkpoint: 'the later visit is indexed without another backup',
      queue: await queue(),
      status,
    })
  } finally {
    service.release()
    await info.attach('progress scope evidence', {
      body: JSON.stringify(
        { observations, queue: await queue().catch(String) },
        null,
        2,
      ),
      contentType: 'application/json',
    })
    const current = await backend<{ config: Record<string, unknown> }>(
      'app_snapshot',
    )
    await backend('save_config', {
      config: { ...current.config, ai: before.config.ai },
    })
    await service.close()
  }
})
