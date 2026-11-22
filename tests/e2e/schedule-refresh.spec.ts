/**
 * Home and an open History search stay up to date when the OS wakes the
 * worker. The worker's reply proves the visits were saved before checking
 * either window. Both windows stay untouched while the backup runs.
 */
import { execFile } from 'node:child_process'
import path from 'node:path'
import { promisify } from 'node:util'
import { expect, test } from '@playwright/test'
import { appendChromeVisits } from '../../scripts/fixtures/synthetic-browsers.mjs'
import type { AppSnapshot, BackupReport } from '../../src/lib/types'
import { backend, fixture, recordPersonalVisits } from './support/fixture'

const exec = promisify(execFile)

test('a scheduled backup refreshes Home and an open History search', async ({
  page,
  context,
}, testInfo) => {
  test.setTimeout(180_000)
  const { config } = await backend<AppSnapshot>('app_snapshot')
  // Use the shortest supported schedule. Once Home shows the previous run
  // in minutes, the new run is both due and distinguishable on the pill.
  await backend('save_config', { config: { ...config, dueAfterHours: 1 / 60 } })
  try {
    await page.goto('/')
    const pill = page.getByRole('link', { name: /^Backed up / })
    await expect(pill).toBeVisible()
    // Words no other spec searches for: the read specs after this one
    // count what their own words find (history-tools' notes use "quokka").
    const title = 'A scheduled visit to Kingfisher Bay'
    const history = await context.newPage()
    await history.goto('/#/browsing/history')
    await history.getByRole('searchbox', { name: 'Search history' }).fill(title)
    await expect(
      history.getByText('Nothing found', { exact: true }).first(),
    ).toBeVisible()

    const visits = [2, 1].map((secondsAgo) => ({
      title,
      url: 'https://scheduled.example/kingfisher-bay',
      at: fixture().now - secondsAgo * 1_000,
      typed: false,
      duration: 500,
    }))
    appendChromeVisits(fixture().files['chrome:Default'], visits)
    await expect(pill).toHaveText(/^Backed up (?!just now)/, {
      timeout: 75_000,
    })

    const target = (
      await exec('rustc', ['--print', 'host-tuple'])
    ).stdout.trim()
    const extension = process.platform === 'win32' ? '.exe' : ''
    const worker = path.resolve(
      'src-tauri/binaries',
      `pathkeep-worker-${target}${extension}`,
    )
    const { stdout } = await exec(worker, ['backup', '--due-only'], {
      timeout: 60_000,
    })
    await testInfo.attach('scheduled worker reply', {
      body: stdout,
      contentType: 'application/json',
    })
    const report: BackupReport = JSON.parse(stdout)
    expect(report).toMatchObject({
      dueSkipped: false,
      run: { status: 'success', newVisits: visits.length },
    })
    recordPersonalVisits(visits)

    await expect(pill).toHaveText(/^Backed up just now/)
    await expect(
      history.getByRole('option').filter({ hasText: title }),
    ).toContainText('2 visits')
  } finally {
    const latest = await backend<AppSnapshot>('app_snapshot')
    await backend('save_config', {
      config: { ...latest.config, dueAfterHours: config.dueAfterHours },
    })
  }
})
