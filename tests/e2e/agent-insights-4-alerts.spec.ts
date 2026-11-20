/**
 * The whole archive's alerts on the Agent space's first page
 * (`data-spaces.md` §3.4): PathKeep opens on the space used last, and
 * browsing can be off, so a failing automatic backup or a keychain that
 * went away must show on Agent's Insights as well as on browsing's Home.
 *
 * Proves:
 * - a failed automatic backup, recorded the way the scheduled worker
 *   records it, shows on Insights with what it means, and its link opens
 *   Settings › Schedule and background work (where the automatic backup
 *   card is), as Home's does; once a later run succeeds the alert is gone;
 * - a keychain that is no longer there shows on Insights too, without a
 *   link (nothing in PathKeep brings it back), when the archive's password
 *   is kept in it;
 * - with nothing wrong, Insights has no alerts at all.
 *
 * Browsing's own alerts (Safari's permission, no browser found) can't be
 * raised on this fixture's Linux runs, so their absence here is not shown.
 * Leaves the keychain available and the last automatic run a success.
 */
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import type { AppSnapshot } from '../../src/lib/types'
import { unlockIfLocked } from './support/app'
import { backend, fixture } from './support/fixture'

const alerts = (page: Page) =>
  page.getByRole('region', { name: 'Needs your attention' })

/** Insights read afresh: the scheduler's status is otherwise kept for a minute. */
async function openInsights(page: Page) {
  await page.goto('/#/agent/insights')
  await page.reload()
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: /^Insights/, level: 1 }),
  ).toBeVisible()
}

/** One run of the scheduled worker, as it records it in the attempt ledger. */
function recordScheduledRun(
  id: string,
  at: Date,
  outcome: 'failed' | 'success',
) {
  const directory = path.join(
    fixture().dirs.projectRoot,
    'schedule',
    'attempts',
  )
  mkdirSync(directory, { recursive: true })
  writeFileSync(
    path.join(directory, `attempt-${id}.json`),
    JSON.stringify({
      id,
      startedAt: at.toISOString(),
      finishedAt: new Date(at.getTime() + 2_000).toISOString(),
      outcome,
      phase: outcome === 'failed' ? 'backup' : 'finished',
      reasonCode: outcome === 'failed' ? 'backup-failed' : 'backup-succeeded',
      detail:
        outcome === 'failed' ? 'Agent conversations could not be read' : null,
      runId: null,
    }),
  )
}

function keychainAvailable(available: boolean) {
  const marker = path.join(fixture().dirs.keyring, 'unavailable')
  if (available) rmSync(marker, { force: true })
  else writeFileSync(marker, '')
}

test('the archive’s alerts show on Agent’s Insights and lead to Agent’s own pages', async ({
  page,
}, testInfo) => {
  await openInsights(page)
  await expect(alerts(page)).toHaveCount(0)

  recordScheduledRun(
    'agent-insights-1',
    new Date(Date.now() - 120_000),
    'failed',
  )
  try {
    await openInsights(page)
    const failed = alerts(page).getByRole('listitem').filter({
      hasText: 'The last automatic backup failed',
    })
    await expect(failed).toContainText(
      'New history is not saved until this is fixed.',
    )
    await testInfo.attach('Insights with an alert', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    await failed.getByRole('link', { name: 'Open schedule settings' }).click()
    await expect(page).toHaveURL(/#\/settings\/background$/)
    await expect(
      page.getByRole('heading', { name: 'Schedule and background work' }),
    ).toBeVisible()
  } finally {
    recordScheduledRun(
      'agent-insights-2',
      new Date(Date.now() - 60_000),
      'success',
    )
  }
  await openInsights(page)
  await expect(alerts(page)).toHaveCount(0)

  const snapshot = await backend<AppSnapshot>('app_snapshot')
  if (
    snapshot.archiveStatus.encrypted &&
    snapshot.config.rememberDatabaseKeyInKeyring
  ) {
    try {
      keychainAvailable(false)
      await openInsights(page)
      const gone = alerts(page).getByRole('listitem').filter({
        hasText: 'The keychain isn’t available',
      })
      await expect(gone).toBeVisible()
      await expect(gone.getByRole('link')).toHaveCount(0)
    } finally {
      keychainAvailable(true)
    }
    await openInsights(page)
    await expect(alerts(page)).toHaveCount(0)
  }
})
