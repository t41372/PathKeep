/**
 * Written before the rescue implementation. Failure modes:
 * - an ID rewind or reuse skips a new visit or changes an old URL;
 * - a local row plus its same-time sync copy is counted twice;
 * - distinct microseconds within one millisecond are collapsed;
 * - parent IDs are resolved across captures or only to earlier input rows;
 * - a v1 archive loses its navigation when old browser rows expire;
 * - a visit whose page row is gone is dropped without a word, or makes the
 *   whole run look broken when everything else was saved;
 * - native URL reuse attaches a search term to yesterday's page;
 * - an unchanged backup or checkpoint replay adds more visits;
 * - an open browser with committed WAL records cannot be backed up.
 *
 * User actions use the real UI. Read-only backend calls compare the actual
 * archive and navigation to the fixture, including details absent from UI.
 * Each step attaches its independent expected/actual data to the trace report.
 */
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { backend, fixture } from './support/fixture'
import {
  addDownload,
  addLegacyMicrosecondSibling,
  addOrphan,
  addTerm,
  keepChromeOpen,
  preciseSafariFile,
  rebuildChrome,
  removeOrphan,
  reuseFirefoxId,
  unixUs,
  type NativeVisit,
} from './support/backup-integrity'

test.describe.configure({ mode: 'serial' })
interface Run {
  id: number
  status: string
  newVisits: number
  newDownloads: number
}
interface Entry {
  id: number
  url: string
  visitTime: number
}
interface Detail {
  run: Run
  artifacts: { kind: string; path: string }[]
  warningDetails: { code: string; profileId?: string; count?: number }[]
}
const prefix = 'https://rescue-new.example/'
const rows = (): NativeVisit[] => [
  {
    id: 1,
    urlId: 1,
    url: `${prefix}start`,
    title: 'Rescue start',
    atUs: unixUs(fixture().now),
    durationUs: null,
    sync: true,
  },
  {
    id: 2,
    urlId: 1,
    url: `${prefix}start`,
    title: 'Rescue start',
    atUs: unixUs(fixture().now),
    durationUs: 30_000_000,
  },
  {
    id: 3,
    urlId: 2,
    url: `${prefix}child`,
    title: 'Rescue child',
    atUs: unixUs(fixture().now + 1000),
    sync: true,
  },
  {
    id: 4,
    urlId: 3,
    url: `${prefix}late-child`,
    title: 'Rescue late child',
    atUs: unixUs(fixture().now + 2000),
    parent: 6,
  },
  {
    id: 5,
    urlId: 4,
    url: `${prefix}precise`,
    title: 'Rescue precise',
    atUs: unixUs(fixture().now + 3000, 101),
  },
  {
    id: 6,
    urlId: 5,
    url: `${prefix}parent`,
    title: 'Rescue parent',
    atUs: unixUs(fixture().now + 1500),
  },
  {
    id: 7,
    urlId: 4,
    url: `${prefix}precise`,
    title: 'Rescue precise',
    atUs: unixUs(fixture().now + 3000, 202),
  },
  {
    id: 8,
    urlId: 2,
    url: `${prefix}child`,
    title: 'Rescue child',
    atUs: unixUs(fixture().now + 1000),
    parent: 2,
  },
]
async function record(
  info: TestInfo,
  name: string,
  actual: unknown,
  expected: unknown,
) {
  await info.attach(name, {
    body: JSON.stringify({ expected, actual }, null, 2),
    contentType: 'application/json',
  })
  expect(actual, name).toEqual(expected)
}
async function snapshot() {
  return backend<{
    recentRuns: Run[]
    archiveStatus: { lastSuccessfulBackupAt: string | null }
  }>('app_snapshot')
}
async function backup(page: Page) {
  const previous = (await snapshot()).recentRuns[0]?.id ?? 0
  await page.goto('/#/backup')
  await page
    .getByRole('button', { name: 'Back up now', exact: true })
    .first()
    .click()
  let run: Run | undefined
  await expect
    .poll(
      async () => {
        run = (await snapshot()).recentRuns.find(
          (item) => item.id > previous && item.status !== 'running',
        )
        return run?.status
      },
      { timeout: 90_000 },
    )
    .not.toBeUndefined()
  return backend<Detail>('load_audit_run_detail', { runId: run!.id })
}
async function entries(profileId = 'chrome:Default') {
  return (
    await backend<{ items: Entry[] }>('query_history', {
      query: { profileId, limit: 150, includeTotal: false },
    })
  ).items
}
const before = () =>
  JSON.parse(
    readFileSync(
      path.join(fixture().dirs.projectRoot, 'expected-before-upgrade.json'),
      'utf8',
    ),
  ) as { navigation: unknown; report: { run: Run } }
let checkpoint: string | undefined

test('opening a healthy v1 archive preserves its 50-step path and historic success', async ({
  page,
}, info) => {
  await page.goto('/#/backup')
  await expect(
    page.getByRole('heading', { name: 'Backup', exact: true }),
  ).toBeVisible()
  const old = before()
  const target = (old.navigation as { targetVisitId: number }).targetVisitId
  await record(
    info,
    'navigation before and after upgrade',
    await backend('get_navigation_path', { visitId: target }),
    old.navigation,
  )
  const historic = await backend<Detail>('load_audit_run_detail', {
    runId: old.report.run.id,
  })
  await record(
    info,
    'historic status',
    historic.run.status,
    old.report.run.status,
  )
  const health = await backend<{ checks: { code: string; ok: boolean }[] }>(
    'doctor_report',
  )
  await record(
    info,
    'all database schema revisions agree with Doctor',
    health.checks.find((check) => check.code === 'schema-version')?.ok,
    true,
  )
  const config = (await backend<{ config: object }>('app_snapshot')).config
  await backend('save_config', { config: { ...config, checkpointDays: 0 } })
})

test('a v1 millisecond match cannot swallow a distinct microsecond after IDs are reordered', async ({
  page,
}, info) => {
  addLegacyMicrosecondSibling()
  const detail = await backup(page)
  await record(
    info,
    'distinct native microsecond event',
    [detail.run.status, detail.run.newVisits],
    ['success', 1],
  )
  await record(
    info,
    'both exact events remain',
    (await entries()).filter(
      (entry) => entry.url === 'https://rescue-old.example/chain/20',
    ).length,
    2,
  )
})

test('rebuilding IDs while Chrome stays open saves new browsing once with exact time and parents', async ({
  page,
}, info) => {
  const native = rows()
  rebuildChrome(native)
  addTerm(1, 'rescue first term')
  addDownload('rescue-download-A', unixUs(fixture().now))
  const open = keepChromeOpen()
  try {
    const detail = await backup(page)
    await record(
      info,
      'fully read source is successful',
      detail.run.status,
      'success',
    )
    await record(
      info,
      'eight native rows are six actual visits',
      detail.run.newVisits,
      6,
    )
    await record(info, 'download after reset', detail.run.newDownloads, 1)
    expect(
      detail.warningDetails.some(
        (warning) => warning.code === 'source-ids-reset',
      ),
    ).toBe(true)
    checkpoint = detail.artifacts.find(
      (artifact) =>
        artifact.kind === 'snapshot' &&
        decodeURIComponent(artifact.path).includes('chrome:Default'),
    )?.path
    const signals = await backend<{
      data: { items: { url: string; foregroundDurationMs: number | null }[] }
    }>('get_observed_interactions', {
      request: {
        dateRange: { start: '2026-10-26', end: '2026-10-29' },
        profileId: 'chrome:Default',
        limit: 512,
      },
    })
    await record(
      info,
      'reused raw ID cannot move a duration to the old event',
      signals.data.items
        .filter((entry) =>
          [prefix + 'start', 'https://rescue-old.example/chain/0'].includes(
            entry.url,
          ),
        )
        .map((entry) => [entry.url, entry.foregroundDurationMs])
        .sort(),
      [
        [prefix + 'start', 30_000],
        ['https://rescue-old.example/chain/0', 20_000],
      ].sort(),
    )
    const saved = await entries()
    const actual = saved
      .filter((entry) => entry.url.startsWith(prefix))
      .map((entry) => [entry.url, entry.visitTime])
      .sort()
    const expected = native
      .filter((row) => row.id !== 2 && row.id !== 8)
      .map((row) => [row.url, Number(row.atUs / 1000n)])
      .sort()
    await record(info, 'actual visits after renumbering', actual, expected)
    for (const [child, parent] of [
      ['child', 'start'],
      ['late-child', 'parent'],
    ]) {
      const target = saved.find((entry) => entry.url === `${prefix}${child}`)!
      const nav = await backend<{ steps: { url: string }[] }>(
        'get_navigation_path',
        { visitId: target.id },
      )
      await record(
        info,
        `parent of ${child}`,
        nav.steps.map((step) => step.url),
        [`${prefix}${parent}`, `${prefix}${child}`],
      )
    }
    const legacyTarget = (before().navigation as { targetVisitId: number })
      .targetVisitId
    await record(
      info,
      'expired browser rows retain legacy navigation',
      await backend('get_navigation_path', { visitId: legacyTarget }),
      before().navigation,
    )
    await page.goto('/#/history')
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill('Rescue precise')
    await expect(
      page.getByText(/^1 page · 2 visits · Full text$/),
    ).toBeVisible()
  } finally {
    open.close()
  }
})

test('higher reused IDs cannot duplicate an event or move old URL and term associations', async ({
  page,
}, info) => {
  rebuildChrome([
    { ...rows()[0], id: 1000, urlId: 42, title: 'Rescue start renamed' },
    {
      id: 1001,
      urlId: 1,
      url: `${prefix}newer`,
      title: 'Rescue newer',
      atUs: unixUs(fixture().now + 5000),
    },
  ])
  addTerm(1, 'rescue second term')
  addDownload('rescue-download-B', unixUs(fixture().now + 5000))
  const detail = await backup(page)
  await record(
    info,
    'one new event despite higher reused IDs',
    [detail.run.status, detail.run.newVisits, detail.run.newDownloads],
    ['success', 1, 1],
  )
  const saved = await entries()
  await record(
    info,
    'old URL still has its original event',
    saved.filter((entry) => entry.url === `${prefix}start`).length,
    1,
  )
  for (const [term, url] of [
    ['rescue first term', `${prefix}start`],
    ['rescue second term', `${prefix}newer`],
  ]) {
    await page.goto('/#/history')
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill(`"${term}"`)
    await expect(
      page
        .getByRole('listbox', { name: 'History results' })
        .getByRole('option'),
    ).toHaveCount(1)
    const found = await backend<{ items: Entry[] }>('query_history', {
      query: { q: `"${term}"`, profileId: 'chrome:Default', limit: 100 },
    })
    await record(
      info,
      term,
      [...new Set(found.items.map((entry) => entry.url))],
      [url],
    )
  }
  const again = await backup(page)
  await record(
    info,
    'unchanged backup',
    [again.run.status, again.run.newVisits, again.run.newDownloads],
    ['success', 0, 0],
  )
})

test('a visit whose page row is gone is counted in a note and the backup still succeeds', async ({
  page,
}, info) => {
  const success = (await snapshot()).archiveStatus.lastSuccessfulBackupAt
  addOrphan()
  try {
    const detail = await backup(page)
    await record(
      info,
      'everything readable was saved',
      detail.run.status,
      'success',
    )
    await record(
      info,
      'the visit without a page is counted for its profile',
      detail.warningDetails
        .filter((warning) => warning.code === 'source-visits-without-page')
        .map((warning) => [warning.profileId, warning.count]),
      [['chrome:Profile 1', 1]],
    )
    expect(
      (await snapshot()).archiveStatus.lastSuccessfulBackupAt,
      'a successful run is the last successful backup',
    ).not.toBe(success)
    await record(
      info,
      'other profile data remains',
      (await entries()).filter((entry) => entry.url.startsWith(prefix)).length,
      7,
    )
    await page.goto(`/#/backup?run=${detail.run.id}`)
    await expect(
      page.getByText(
        /^1 entry in .+’s history has no page in the browser’s file, so it has no address to save\.$/,
      ),
    ).toBeVisible()
  } finally {
    removeOrphan()
  }
})

test('Firefox reuses a deleted ID without losing either visit', async ({
  page,
}, info) => {
  reuseFirefoxId(unixUs(fixture().now + 9000))
  const detail = await backup(page)
  await record(
    info,
    'Firefox reuse backup',
    [detail.run.status, detail.run.newVisits],
    ['success', 1],
  )
  await record(
    info,
    'Firefox old and new events',
    (await entries('firefox:k3x9.default-release'))
      .map((entry) => entry.url)
      .sort(),
    [
      'https://rescue-firefox.example/new',
      'https://rescue-firefox.example/old',
    ],
  )
})

test('replaying the captured source with sync copies uses the same event identity', async ({
  page,
}, info) => {
  expect(checkpoint, 'backup retained its browser checkpoint').toBeTruthy()
  const sourceHash = createHash('sha256')
    .update(readFileSync(path.join(checkpoint!, 'History')))
    .digest('hex')
  const request = { snapshotPath: checkpoint }
  const preview = await backend<{ estimatedVisits: number }>(
    'preview_snapshot_restore',
    { request },
  )
  await record(
    info,
    'checkpoint contains every native row',
    preview.estimatedVisits,
    rows().length,
  )
  const restored = await backend<{ run: Run }>('run_snapshot_restore', {
    request,
  })
  await record(
    info,
    'replay does not create copies',
    [restored.run.status, restored.run.newVisits],
    ['success', 0],
  )
  await record(
    info,
    'archive still has exactly seven new visits',
    (await entries()).filter((entry) => entry.url.startsWith(prefix)).length,
    7,
  )
  expect(
    createHash('sha256')
      .update(readFileSync(path.join(checkpoint!, 'History')))
      .digest('hex'),
  ).toBe(sourceHash)
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill('Rescue precise')
  await expect(page.getByText(/^1 page · 2 visits · Full text$/)).toBeVisible()
})

test('Safari retains two events whose exact timestamps differ within one microsecond', async ({
  page,
}, info) => {
  const file = preciseSafariFile()
  await page.goto('/#/backup')
  const field = page.getByRole('textbox', {
    name: 'Path to a history file or profile folder',
  })
  await field.fill(file)
  await field.press('Enter')
  await expect(page.getByTestId('browser-import-preview')).toContainText(
    '2 new visits',
  )
  await page
    .getByRole('button', { name: 'Import 2 visits', exact: true })
    .click()
  await expect(page.getByText('Imported 2 new visits').first()).toBeVisible({
    timeout: 60_000,
  })
  const saved = await backend<{ items: Entry[] }>('query_history', {
    query: { domain: 'rescue-safari.example', limit: 100, includeTotal: false },
  })
  await record(info, 'two exact Safari events', saved.items.length, 2)
  await page
    .getByRole('button', { name: 'Import another', exact: true })
    .click()
  await field.fill(file)
  await field.press('Enter')
  await expect(page.getByTestId('browser-import-preview')).toContainText(
    '0 new visits',
  )
  await page.goto('/#/history')
  await page
    .getByRole('searchbox', { name: 'Search history' })
    .fill('Safari precise visits')
  await expect(page.getByText(/^1 page · 2 visits · Full text$/)).toBeVisible()
})
