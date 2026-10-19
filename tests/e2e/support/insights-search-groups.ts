/**
 * Responsibilities: prove the optional Insights card against imported history and a real index.
 * Not responsible for: replacing grouping or reaching publication races (Rust tests cover those).
 * Failure modes: a build in the opposite order never warms months; profiles leak; strips count
 * searches as days; ranges disappear in links; removing a service removes deterministic cards.
 * Tests: invoked by semantic-index.spec.ts; attaches rows, fixture and light/dark card screenshots.
 */
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { expect, type Page, type TestInfo } from '@playwright/test'
import { writeChromeHistory } from '../../../scripts/fixtures/synthetic-browsers.mjs'
import type { SearchGroupsCard } from '../../../src/lib/backend-client/search-groups'
import { backend, fixture } from './fixture'
import { unlockIfLocked } from './app'
import { importRelatedFile, undoRelatedFile } from './related-pages'
import { startFakeEmbeddings } from './fake-embeddings'
import {
  corpus,
  rebuildInsights,
  searchVector,
  SUBJECT_A,
  SUBJECT_B,
} from './search-groups'

async function visit(page: Page, url: string) {
  await page.goto(url)
  await unlockIfLocked(page)
}
const card = (page: Page) =>
  page
    .getByRole('heading', { name: 'Things you looked into', exact: true })
    .locator('xpath=ancestor::section[1]')
async function monthInCard(page: Page, key: string) {
  const month = card(page).locator(`[data-search-month="${key}"]`)
  await expect
    .poll(
      async () => {
        if (await month.count()) return true
        await card(page)
          .locator('.overflow-y-auto')
          .evaluate((element) => element.scrollBy(0, 200))
        return false
      },
      { timeout: 30000 },
    )
    .toBe(true)
  return month
}
const january = { start: '2001-01-01', end: '2001-01-30' }
const year = { start: '2001-01-01', end: '2001-12-31' }
const url = (range: typeof january) =>
  `/#/insights?range=custom&start=${range.start}&end=${range.end}`

export async function insightsSearchGroupsScenario(
  page: Page,
  testInfo: TestInfo,
) {
  const before = (
    await backend<{ config: { ai: Record<string, unknown> } }>('app_snapshot')
  ).config
  const service = await startFakeEmbeddings({
    dimensions: 64,
    vectorFor: searchVector,
  })
  const folder = path.join(fixture().root, 'insights-search-groups')
  mkdirSync(folder, { recursive: true })
  const januaryVisits = corpus().filter((v) => v.at >= Date.UTC(2001, 0, 1))
  const primary = [
    ...januaryVisits,
    ...januaryVisits.map((v) => ({ ...v, at: v.at + 31 * 86400000 })),
  ]
  // A second browser repeats only the bread subject, at other instants. Counts must change when
  // selecting the first browser, while both subjects and their days remain.
  const breadVisits = januaryVisits
    .filter(
      (v) =>
        v.title?.startsWith('Search group bread') ||
        SUBJECT_B.includes(v.term ?? ''),
    )
    .map((v) => ({ ...v, at: v.at + 4 * 3600000 }))
  const secondary = [
    ...breadVisits,
    ...breadVisits.map((v) => ({ ...v, at: v.at + 31 * 86400000 })),
  ]
  const files = ['subjects-primary.sqlite', 'subjects-secondary.sqlite'].map(
    (name) => path.join(folder, name),
  )
  writeChromeHistory(files[0], primary)
  writeChromeHistory(files[1], secondary)
  const imported: string[] = []
  const sourceStats = () =>
    backend<
      {
        profileId: string
        browserName: string
        profileName: string
        visitCount: number
      }[]
    >('load_source_stats')
  // localStorage is only readable once the app's page is open.
  await visit(page, '/#/')
  const savedTheme = await page.evaluate(() =>
    localStorage.getItem('pathkeep-theme'),
  )
  await testInfo.attach('search card fixture', {
    body: JSON.stringify({ primary, secondary }, null, 2),
    contentType: 'application/json',
  })
  try {
    await backend('save_config', {
      config: { ...before, ai: { ...before.ai, autoIndexAfterBackup: false } },
    })
    const previousProfiles = new Set(
      (await sourceStats()).map((p) => p.profileId),
    )
    await importRelatedFile(page, files[0], primary.length)
    imported.push(files[0])
    const primaryProfile = (await sourceStats()).find(
      (p) => !previousProfiles.has(p.profileId),
    )!
    expect(primaryProfile).toBeTruthy()
    await importRelatedFile(page, files[1], secondary.length)
    imported.push(files[1])
    await rebuildInsights(page)
    await visit(page, '/#/settings/ai')
    await page.getByRole('combobox', { name: 'Model' }).click()
    await page
      .getByRole('option', { name: 'Add an embeddings service…' })
      .click()
    const dialog = page.getByRole('dialog', {
      name: 'Add an embeddings service',
    })
    await dialog.getByRole('combobox', { name: 'Service' }).click()
    await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
    await dialog.getByRole('textbox', { name: 'Address' }).fill(service.url)
    await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-embed')
    await dialog
      .getByRole('textbox', { name: 'Name' })
      .fill('E2E Insights groups')
    await dialog
      .getByRole('button', { name: 'Add service', exact: true })
      .click()
    await expect(dialog).toBeHidden()
    const enabled = page.getByRole('switch', {
      name: 'Semantic search',
      exact: true,
    })
    if (!(await enabled.isChecked())) {
      await enabled.click()
      await expect(enabled).toBeChecked()
    } else
      await page
        .getByRole('button', { name: 'Build index', exact: true })
        .click()
    await expect
      .poll(
        async () => {
          const queue = await backend<{
            running: number
            queued: number
            semanticIndex: { chosenComplete: boolean }
          }>('load_ai_queue_status')
          return (
            queue.semanticIndex.chosenComplete &&
            queue.running + queue.queued === 0
          )
        },
        { timeout: 240000 },
      )
      .toBe(true)
    // This specifically observes warmed month rows, not an on-demand custom-range computation.
    await expect
      .poll(
        async () => {
          const result = await backend<SearchGroupsCard>('get_search_groups', {
            request: { dateRange: year, profileId: null },
          })
          return result.data
            .filter(
              (w) =>
                w.range.start.startsWith('2001-0') &&
                ['01', '02'].includes(w.range.start.slice(5, 7)),
            )
            .map((w) => ({
              start: w.range.start,
              updating: w.updating,
              groups: w.groups.length,
            }))
        },
        { timeout: 240000 },
      )
      .toEqual([
        { start: '2001-02-01', updating: false, groups: 2 },
        { start: '2001-01-01', updating: false, groups: 2 },
      ])
    const requests = service.requests()
    await visit(page, url(january))
    await expect(card(page).locator('[data-search-group]')).toHaveCount(2)
    const camera = card(page)
      .locator('[data-search-group]')
      .filter({ hasText: SUBJECT_A[0] })
    const bread = card(page)
      .locator('[data-search-group]')
      .filter({ hasText: SUBJECT_B.at(-1)! })
    await expect(camera).toContainText('4 searches · 3 days · 2 pages opened')
    await expect(bread).toContainText('32 searches · 2 days · 1 page opened')
    await expect(bread).toContainText('and 13 other searches')
    expect(
      await camera
        .locator('[data-searched="true"]')
        .evaluateAll((cells) => cells.map((c) => c.getAttribute('data-date'))),
    ).toEqual(['2001-01-10', '2001-01-11', '2001-01-12'])
    await expect(camera.locator('[data-date]')).toHaveCount(30)
    await expect(
      camera.locator('[data-date="2001-01-10"]'),
    ).toHaveAccessibleName(/Jan 10, 2001: searched/)
    await camera.getByRole('link').click()
    await unlockIfLocked(page)
    await expect(
      page.getByRole('heading', { name: `“${SUBJECT_A[0]}”`, exact: true }),
    ).toBeVisible()
    expect(page.url()).toContain('start=2001-01-01')
    expect(page.url()).toContain('end=2001-01-30')
    await expect(
      page.getByRole('heading', { name: 'Similar searches', exact: true }),
    ).toBeVisible()
    await visit(page, url(year))
    const february = await monthInCard(page, '2001-02')
    await expect(february.locator('[data-search-group]')).toHaveCount(2)
    await expect(card(page)).not.toContainText('Updating…')
    await expect(card(page)).toContainText('Whole calendar months')
    const januaryMonth = card(page).locator('[data-search-month="2001-01"]')
    await januaryMonth.scrollIntoViewIfNeeded()
    await expect(januaryMonth.locator('[data-search-group]')).toHaveCount(2)
    await visit(page, url(january))
    await page.getByRole('combobox', { name: 'Browser profile' }).click()
    // Imported profiles have the same default display name; the larger primary one is first.
    await page
      .getByRole('option', {
        name: `${primaryProfile.browserName} · ${primaryProfile.profileName}`,
        exact: true,
      })
      .first()
      .click()
    await unlockIfLocked(page)
    expect(
      new URL(page.url().split('#')[1], 'http://local').searchParams.get(
        'profile',
      ),
    ).toBe(primaryProfile.profileId)
    await expect(
      card(page)
        .locator('[data-search-group]')
        .filter({ hasText: SUBJECT_B.at(-1)! }),
    ).toContainText('16 searches · 2 days · 1 page opened')
    await expect(card(page).locator('[data-search-group]')).toHaveCount(2)
    await camera.getByRole('link').click()
    await unlockIfLocked(page)
    expect(
      new URL(page.url().split('#')[1], 'http://local').searchParams.get(
        'profile',
      ),
    ).toBe(primaryProfile.profileId)
    await visit(
      page,
      `${url(year)}&profile=${encodeURIComponent(primaryProfile.profileId)}`,
    )
    await expect(card(page)).toContainText(
      'Monthly groups include all browsers.',
    )
    const allBrowserMonth = await monthInCard(page, '2001-02')
    await expect(
      allBrowserMonth
        .locator('[data-search-group]')
        .filter({ hasText: SUBJECT_B.at(-1)! }),
    ).toContainText('32 searches · 2 days · 1 page opened')
    await visit(page, url(january))
    for (const theme of ['light', 'dark']) {
      await page.evaluate(
        (value) => localStorage.setItem('pathkeep-theme', value),
        theme,
      )
      // The theme is applied once when the page loads; a hash change keeps the old one.
      await page.reload()
      await unlockIfLocked(page)
      await expect
        .poll(() =>
          page.evaluate(() =>
            document.documentElement.classList.contains('dark'),
          ),
        )
        .toBe(theme === 'dark')
      await visit(page, url(january))
      await expect(card(page).locator('[data-search-group]')).toHaveCount(2)
      await testInfo.attach(
        `Things you looked into (${theme}, browser with real backend)`,
        { body: await card(page).screenshot(), contentType: 'image/png' },
      )
    }
    expect(service.requests()).toBe(requests)
    const result = await backend<SearchGroupsCard>('get_search_groups', {
      request: { dateRange: january, profileId: null },
    })
    await testInfo.attach('search card rows and imported visits', {
      body: JSON.stringify({ result, primary, secondary }, null, 2),
      contentType: 'application/json',
    })
    const frequentBefore = await backend('get_frequent_searches', {
      request: { dateRange: january, profileId: null, limit: 10, cursor: null },
    })
    await visit(page, '/#/settings/ai')
    await page
      .getByRole('button', { name: 'Remove', exact: true })
      .first()
      .click()
    const remove = page.getByRole('alertdialog', {
      name: 'Remove E2E Insights groups?',
    })
    await remove.getByRole('button', { name: 'Remove', exact: true }).click()
    await expect(remove).toBeHidden()
    await visit(page, url(january))
    await expect(
      page.getByRole('heading', {
        name: 'Things you looked into',
        exact: true,
      }),
    ).toHaveCount(0)
    await expect(
      page.getByRole('heading', { name: 'What you search for', exact: true }),
    ).toBeVisible()
    expect(
      await backend('get_frequent_searches', {
        request: {
          dateRange: january,
          profileId: null,
          limit: 10,
          cursor: null,
        },
      }),
    ).toEqual(frequentBefore)
  } finally {
    try {
      for (const file of imported.reverse())
        await undoRelatedFile(page, path.basename(file))
      const current = (
        await backend<{ config: { ai: Record<string, unknown> } }>(
          'app_snapshot',
        )
      ).config
      await backend('save_config', { config: { ...current, ai: before.ai } })
      await page.evaluate(
        (value) =>
          value === null
            ? localStorage.removeItem('pathkeep-theme')
            : localStorage.setItem('pathkeep-theme', value),
        savedTheme,
      )
    } finally {
      await service.close()
    }
  }
}
