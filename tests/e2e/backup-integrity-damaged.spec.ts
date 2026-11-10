/**
 * A native v1 archive actually damaged by the baseline's false-success backup.
 * Failure modes: a reused ID still hides missing visits; a damaged old URL is
 * mistaken for the new event; sync copies inflate the recovery; a second run
 * appends another copy; historic success is silently rewritten.
 * The fixture was captured before production changes, with real app commands.
 */
import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { backend, fixture, localDay } from './support/fixture'

test('the next backup fills a v1 false-success gap and an unchanged rerun stays empty', async ({
  page,
}, info) => {
  const old = JSON.parse(
    readFileSync(
      path.join(fixture().dirs.projectRoot, 'expected-before-upgrade.json'),
      'utf8',
    ),
  )
  const source = new DatabaseSync(fixture().files['chrome:Default'], {
    readOnly: true,
  })
  const query = source.prepare(
    'SELECT u.url,v.visit_time FROM visits v JOIN urls u ON u.id=v.url',
  )
  query.setReadBigInts(true)
  const rows = query.all() as { url: string; visit_time: bigint }[]
  source.close()
  const expected = [
    ...new Map(
      rows.map((row) => [
        `${row.url}:${row.visit_time}`,
        [row.url, Number((row.visit_time - 11_644_473_600_000_000n) / 1000n)],
      ]),
    ).values(),
  ].sort()
  const current = async () =>
    (
      await backend<{ items: { url: string; visitTime: number }[] }>(
        'query_history',
        {
          query: {
            profileId: 'chrome:Default',
            startTimeMs: fixture().now,
            limit: 100,
            includeTotal: false,
          },
        },
      )
    ).items
      .map((row) => [row.url, row.visitTime])
      .sort()
  await page.goto('/#/history')
  await expect(
    page.getByRole('searchbox', { name: 'Search history' }),
  ).toBeVisible()
  expect(await current()).toEqual([])
  let previous = old.report.run.id as number
  for (const added of [expected.length, 0]) {
    await page.goto('/#/backup')
    await page
      .getByRole('button', { name: 'Back up now', exact: true })
      .first()
      .click()
    let result: { id: number; status: string; newVisits: number } | undefined
    await expect
      .poll(
        async () => {
          const snapshot = await backend<{
            recentRuns: { id: number; status: string; newVisits: number }[]
          }>('app_snapshot')
          result = snapshot.recentRuns.find(
            (run) => run.id > previous && run.status !== 'running',
          )
          return result?.status
        },
        { timeout: 90_000 },
      )
      .toBe('success')
    expect(result!.newVisits).toBe(added)
    previous = result!.id
    const actual = await current()
    await info.attach(`recovered-events-${added}`, {
      body: JSON.stringify({ expected, actual }, null, 2),
      contentType: 'application/json',
    })
    expect(actual).toEqual(expected)
  }
  const historic = await backend<{ run: { status: string } }>(
    'load_audit_run_detail',
    { runId: old.report.run.id },
  )
  expect(historic.run.status).toBe(old.report.run.status)
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill(`Rescue precise after:${localDay(fixture().now)}`)
  await expect(page.getByText(/^1 page · 2 visits · Full text$/)).toBeVisible()
})
