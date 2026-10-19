/**
 * Responsibilities: exercise search grouping through imported history, real indexes and Ask.
 * Not responsible for: replacing database reads or implementing similarity in the test.
 * Failure modes: shared words mask missing vectors; counts omit short/navigational searches;
 * opened pages come from outside trails; a second pass embeds completed searches again.
 * Tests: invoked by semantic-index.spec.ts; saves tool reports, rows and a screenshot.
 */
import { mkdirSync } from 'node:fs'
import { createHash } from 'node:crypto'
import path from 'node:path'
import { expect, type Page, type TestInfo } from '@playwright/test'
import { writeChromeHistory } from '../../../scripts/fixtures/synthetic-browsers.mjs'
import { addLmStudioService } from './ai-service'
import { unlockIfLocked } from './app'
import { startFakeEmbeddings } from './fake-embeddings'
import { offersTools, startFakeModel, toolResults } from './fake-model'
import { backend, fixture, type FixtureVisit } from './fixture'
import { importRelatedFile, undoRelatedFile } from './related-pages'
import type { SimilarSearches } from '../../../src/lib/backend-client/similar-searches'

/** Opens a screen; App Lock can engage during the long index waits between steps. */
async function visit(page: Page, url: string) {
  await page.goto(url)
  await unlockIfLocked(page)
}

export const SUBJECT_A = ['lumincam', 'portable stills', 'travel sensor']
export const SUBJECT_B = [
  'sourdough starter',
  'rye feeding',
  'wild yeast',
  'bread levain',
  'fermentation jar',
  'flour hydration',
  'dough proofing',
  'baking culture',
  'wheat inoculation',
  'loaf schedule',
  'starter discard',
  'wholemeal refresh',
  'levain temperature',
  'acetic balance',
  'bread acidity',
  'lactic growth',
]
const SHORT = 'x'
const NOT_INDEXED = 'unindexed optics'
const UNDONE = 'hidden camera wording'
const NAVIGATION = 'https://groups-navigation.example.com/'
const QUERIES = [...SUBJECT_A, ...SUBJECT_B, SHORT]
// Keep the all-time 99th percentile separate from the window's 98th percentile.
const OUTSIDE_WINDOW = Array.from(
  { length: 200 },
  (_, i) => `outside window query ${i}`,
)
const EMBEDDED_QUERIES = [...QUERIES, ...OUTSIDE_WINDOW]
const START = '2001-01-10'
const END = '2001-01-14'
const A_PAGES = [
  {
    url: 'https://groups-camera.example.com/one',
    title: 'Search group camera one',
  },
  {
    url: 'https://groups-review.example.net/two',
    title: 'Search group camera two',
  },
]
const B_PAGE = {
  url: 'https://groups-bread.example.org/starter',
  title: 'Search group bread starter',
}

export function corpus(): FixtureVisit[] {
  const visits: FixtureVisit[] = []
  const search = (query: string, at: number, pages = A_PAGES) => {
    visits.push({
      url: `https://www.google.com/search?q=${encodeURIComponent(query)}`,
      title: `${query} - Google Search`,
      term: query,
      at,
      typed: true,
      duration: 1000,
    })
    pages.forEach((page, index) =>
      visits.push({
        ...page,
        at: at + (index + 1) * 10_000,
        typed: false,
        duration: 1000,
      }),
    )
  }
  const first = new Date(`${START}T12:00:00Z`).getTime()
  SUBJECT_A.forEach((query, day) => search(query, first + day * 86_400_000))
  // The same wording is searched again after its pages, in the same trail: two searches, one trail.
  search(SUBJECT_A[0], first + 60_000)
  SUBJECT_B.forEach((query, index) =>
    search(
      query,
      first +
        (index % 2) * 86_400_000 +
        3_600_000 +
        Math.floor(index / 2) * 25 * 60_000,
      [B_PAGE],
    ),
  )
  OUTSIDE_WINDOW.forEach((query, index) =>
    search(query, Date.UTC(1991, 0, 1, 12) + index * 60_000, []),
  )
  search(SHORT, first + 4 * 3_600_000, [])
  search(NAVIGATION, first + 5 * 3_600_000, [])
  // Same month, outside every search trail: it must never appear in the report's pages.
  visits.push({
    url: 'https://groups-outside.example.com/unrelated',
    title: 'Unrelated visit outside every search trail',
    at: first + 6 * 3_600_000,
    typed: true,
    duration: 1000,
  })
  return visits.sort((a, b) => a.at - b.at)
}

type Group = {
  id: string
  wordings: {
    query: string
    normalizedQuery: string
    searches: number
    trails: number
  }[]
  trails: number
  searches: number
  days: { count: number; first: string; last: string }
  pages: { historyId: number; url: string; visits: number }[]
  searchEvents?: { historyId: number; query: string }[]
}
type Report = {
  distinctSearches: number
  searches: number
  short: { count: number; searches: number; examples: string[] }
  notIndexed: { count: number; searches: number }
  notGrouped: { count: number; searches: number }
  groups: Group[]
}
type Request = {
  report: 'searches'
  start_date: string
  end_date: string
  group?: string
  output_cursor?: string
}

const config = async () =>
  (await backend<{ config: { ai: Record<string, unknown> } }>('app_snapshot'))
    .config
const idle = async () =>
  expect
    .poll(
      async () => {
        const queue = await backend<{ running: number; queued: number }>(
          'load_ai_queue_status',
        )
        return queue.running + queue.queued
      },
      { timeout: 240_000 },
    )
    .toBe(0)

export async function rebuildInsights(page: Page) {
  await visit(page, '/#/settings/background')
  await page.getByRole('button', { name: 'Rebuild', exact: true }).click()
  const dialog = page.getByRole('alertdialog', { name: 'Rebuild Insights?' })
  await dialog.getByRole('button', { name: 'Rebuild', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByText('Rebuild queued.', { exact: true })).toBeVisible()
  await expect
    .poll(
      async () => {
        const runtime = await backend<{
          queue: { running: number; queued: number }
        }>('load_intelligence_runtime')
        return runtime.queue.running + runtime.queue.queued
      },
      { timeout: 240_000 },
    )
    .toBe(0)
}

/**
 * A's raw pair similarity is .6, below B's .7. With their opened pages it is about
 * .887 versus .872: omitting page context fails the grouping assertion. Sixteen B
 * wordings put the window's cut below A's three pairs. Other all-time searches use
 * orthogonal dimensions, keeping their own percentile independent of the window.
 */
export function searchVector(text: string, dimensions: number): number[] {
  const vector = new Array<number>(dimensions).fill(0)
  const a = text === UNDONE ? SUBJECT_A.length : SUBJECT_A.indexOf(text)
  const b = SUBJECT_B.indexOf(text)
  if (a >= 0) {
    vector[0] = Math.sqrt(0.6)
    vector[3 + a] = Math.sqrt(0.4)
  } else if (b >= 0) {
    vector[1] = Math.sqrt(0.7)
    vector[10 + b] = Math.sqrt(0.3)
  } else if (text.startsWith('Search group camera')) {
    vector[0] = 1
  } else if (text.startsWith('Search group bread')) {
    vector[1] = 0.2
    vector[2] = Math.sqrt(0.96)
  } else {
    const bytes = createHash('sha256').update(text).digest()
    for (let i = 0; i < bytes.length; i++) vector[32 + i] = bytes[i] / 127.5 - 1
  }
  return vector
}

/** The report must use actual opened pages to reunite the unrelated wordings. */
export async function searchGroupsScenario(page: Page, testInfo: TestInfo) {
  const before = await config()
  const folder = path.join(fixture().root, 'search-groups')
  mkdirSync(folder, { recursive: true })
  const file = path.join(folder, 'keyword-groups.sqlite')
  const visits = corpus()
  writeChromeHistory(file, visits)
  const embeddings = await startFakeEmbeddings({
    dimensions: 64,
    vectorFor: searchVector,
  })
  const base: Request = { report: 'searches', start_date: START, end_date: END }
  const reports: { request: Request; text: string; value?: Report }[] = []
  let task: Request = base
  let fragments = ''
  let pending: Request[] = []
  const answer = 'Search grouping evidence is ready.'
  const model = await startFakeModel({
    apiKey: null,
    models: [{ id: 'search-groups-chat', loaded_context_length: 262_144 }],
    script: (body) => {
      if (!offersTools(body)) return { content: 'OK' }
      if (body.messages?.at(-1)?.role !== 'tool') {
        task = base
        fragments = ''
        pending = [{ ...base, start_date: '2000-12-06' }]
      } else {
        const text = toolResults(body).at(-1)!
        let value: Report | undefined
        try {
          const parsed = JSON.parse(
            text.slice(text.indexOf('\n') + 1),
          ) as Report & { jsonFragment?: string; nextOutputCursor?: string }
          if (parsed.jsonFragment !== undefined) {
            fragments += parsed.jsonFragment
            if (parsed.nextOutputCursor) {
              task = { ...task, output_cursor: parsed.nextOutputCursor }
              return {
                toolCalls: [{ name: 'intelligence_report', arguments: task }],
              }
            }
            value = JSON.parse(fragments) as Report
          } else value = parsed
        } catch {
          /* A refusal is plain text, retained in the artifact below. */
        }
        reports.push({ request: task, text, value })
        if (value?.groups && !task.group)
          pending.unshift(
            ...value.groups.map((group) => ({ ...base, group: group.id })),
          )
        const next = pending.shift()
        if (!next) return { content: answer }
        task = next
        fragments = ''
      }
      return { toolCalls: [{ name: 'intelligence_report', arguments: task }] }
    },
  })
  const imported: string[] = []
  let completed = false
  try {
    await visit(page, '/#/settings/ai')
    await unlockIfLocked(page)
    await idle()
    const current = await config()
    await backend('save_config', {
      config: {
        ...current,
        ai: { ...current.ai, autoIndexAfterBackup: false },
      },
    })
    await importRelatedFile(page, file, visits.length)
    imported.push(path.basename(file))
    const hiddenFile = path.join(folder, 'undone-keyword.sqlite')
    writeChromeHistory(hiddenFile, [
      {
        url: `https://www.google.com/search?q=${encodeURIComponent(UNDONE)}`,
        title: `${UNDONE} - Google Search`,
        term: UNDONE,
        at: new Date('1990-01-01T12:00:00Z').getTime(),
        typed: true,
        duration: 1000,
      },
    ])
    await importRelatedFile(page, hiddenFile, 1)
    imported.push(path.basename(hiddenFile))
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
    await dialog.getByRole('textbox', { name: 'Address' }).fill(embeddings.url)
    await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-embed')
    await dialog
      .getByRole('textbox', { name: 'Name' })
      .fill('E2E search group embeddings')
    await dialog
      .getByRole('button', { name: 'Add service', exact: true })
      .click()
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'E2E search group embeddings',
    )
    const enabled = page.getByRole('switch', {
      name: 'Semantic search',
      exact: true,
    })
    if (!(await enabled.isChecked())) await enabled.click()
    else
      await page
        .getByRole('button', { name: 'Build index', exact: true })
        .click()
    await expect
      .poll(
        async () =>
          (
            await backend<{ semanticIndex: { chosenComplete: boolean } }>(
              'load_ai_queue_status',
            )
          ).semanticIndex.chosenComplete,
        { timeout: 240_000 },
      )
      .toBe(true)
    await idle()
    const sent = () =>
      embeddings.inputs.filter((text) => EMBEDDED_QUERIES.includes(text))
    expect(sent().sort()).toEqual([...EMBEDDED_QUERIES].sort())
    expect(
      embeddings.inputs.some((text) =>
        text.startsWith(`${SUBJECT_A[0]} - Google Search\n`),
      ),
    ).toBe(true)
    expect(embeddings.inputs.filter((text) => text === UNDONE)).toHaveLength(1)
    const beforeUndo = await backend<SimilarSearches>('get_similar_searches', {
      query: SUBJECT_A[0],
      wordings: [],
    })
    expect(beforeUndo.state).toBe('ready')
    if (beforeUndo.state === 'ready')
      expect(beforeUndo.items.map((row) => row.query)).toContain(UNDONE)
    // Hold the old Insights generation deliberately: visibility must not wait for its rebuild.
    await visit(page, '/#/settings/background')
    // The switch flips once the setting is saved, so wait for it rather than uncheck().
    const pause = page.getByRole('switch', {
      name: 'Run background work',
      exact: true,
    })
    await pause.click()
    await expect(pause).not.toBeChecked()
    await expect(
      page.getByText('Background work paused.', { exact: true }),
    ).toBeVisible()
    await undoRelatedFile(page, path.basename(hiddenFile))
    imported.splice(imported.indexOf(path.basename(hiddenFile)), 1)
    const afterUndo = await backend<SimilarSearches>('get_similar_searches', {
      query: SUBJECT_A[0],
      wordings: [],
    })
    expect(afterUndo.state).toBe('ready')
    if (afterUndo.state === 'ready')
      expect(afterUndo.items.map((row) => row.query).sort()).toEqual(
        SUBJECT_A.slice(1).sort(),
      )
    await testInfo.attach('search visibility before Insights catches up', {
      body: JSON.stringify({ beforeUndo, afterUndo }, null, 2),
      contentType: 'application/json',
    })
    await visit(page, '/#/settings/background')
    const resume = page.getByRole('switch', {
      name: 'Run background work',
      exact: true,
    })
    await resume.click()
    await expect(resume).toBeChecked()
    await expect(
      page.getByText('Background work resumed.', { exact: true }),
    ).toBeVisible()
    // There is no incremental-update button for an already complete index. Exercise its real command.
    await backend('build_ai_index', {
      request: { fullRebuild: false, clearOnly: false, scope: 'incremental' },
    })
    await idle()
    expect(sent().sort()).toEqual([...EMBEDDED_QUERIES].sort())
    expect(embeddings.inputs).not.toContain(NAVIGATION)
    const lateFile = path.join(folder, 'unindexed-keyword.sqlite')
    writeChromeHistory(lateFile, [
      {
        url: `https://www.google.com/search?q=${encodeURIComponent(NOT_INDEXED)}`,
        title: `${NOT_INDEXED} - Google Search`,
        term: NOT_INDEXED,
        at: new Date(`${END}T12:00:00Z`).getTime(),
        typed: true,
        duration: 1000,
      },
    ])
    await importRelatedFile(page, lateFile, 1)
    imported.push(path.basename(lateFile))
    await rebuildInsights(page)
    expect(embeddings.inputs).not.toContain(NOT_INDEXED)
    await addLmStudioService(page, {
      name: 'E2E search group chat',
      address: model.url.replace(/\/v1$/, ''),
      model: 'search-groups-chat',
    })
    await visit(page, '/#/ask')
    const composer = page.getByRole('textbox', { name: 'Message' })
    await composer.fill(
      'Group these searches, read every group, and check the longer window.',
    )
    await composer.press('Enter')
    await expect(
      page.getByRole('main').locator('aside + div').getByText(answer).last(),
    ).toBeVisible({ timeout: 120_000 })
    const report = reports.find(
      (result) => result.value?.groups && !result.request.group,
    )?.value
    expect(report).toBeTruthy()
    expect(report!.distinctSearches).toBe(QUERIES.length + 1)
    expect(report!.searches).toBe(QUERIES.length + 2)
    expect(report!.short).toMatchObject({
      count: 1,
      searches: 1,
      examples: [SHORT],
    })
    expect(report!.notIndexed).toMatchObject({ count: 1, searches: 1 })
    expect(report!.notGrouped.count).toBe(0)
    expect(report!.groups).toHaveLength(2)
    const grouped = report!.groups.flatMap((group) =>
      group.wordings.map((row) => row.normalizedQuery),
    )
    expect(grouped.sort()).toEqual([...SUBJECT_A, ...SUBJECT_B].sort())
    expect(new Set(grouped).size).toBe(grouped.length)
    expect(grouped).not.toContain(NAVIGATION)
    expect(
      report!.groups.reduce(
        (sum, group) => sum + group.wordings.length,
        report!.short.count +
          report!.notIndexed.count +
          report!.notGrouped.count,
      ),
    ).toBe(report!.distinctSearches)
    expect(
      report!.groups.reduce(
        (sum, group) => sum + group.searches,
        report!.short.searches +
          report!.notIndexed.searches +
          report!.notGrouped.searches,
      ),
    ).toBe(report!.searches)
    const camera = report!.groups.find((group) =>
      group.wordings.some((row) => row.query === SUBJECT_A[0]),
    )!
    expect(camera.wordings.map((row) => row.query)).toEqual(
      [...SUBJECT_A].reverse(),
    )
    expect(camera.wordings.map((row) => row.query).sort()).toEqual(
      [...SUBJECT_A].sort(),
    )
    expect(camera).toMatchObject({
      id: 's1',
      trails: 3,
      searches: 4,
      days: { count: 3, first: START, last: '2001-01-12' },
    })
    expect(camera.pages.map((row) => [row.url, row.visits]).sort()).toEqual(
      A_PAGES.map((page) => [page.url, 4]).sort(),
    )
    const bread = report!.groups.find((group) => group.id !== camera.id)!
    expect(bread.wordings.map((row) => row.query).sort()).toEqual(
      [...SUBJECT_B].sort(),
    )
    expect(bread.pages.map((row) => row.url)).toEqual([B_PAGE.url])
    const details = reports.filter((result) => result.request.group)
    expect(details).toHaveLength(2)
    expect(
      details
        .flatMap((result) => result.value!.groups[0].searchEvents!)
        .map((row) => row.historyId).length,
    ).toBe(SUBJECT_A.length + SUBJECT_B.length + 1)
    expect(
      new Set(
        details
          .flatMap((result) => result.value!.groups[0].searchEvents!)
          .map((row) => row.historyId),
      ).size,
    ).toBe(SUBJECT_A.length + SUBJECT_B.length + 1)
    expect(
      reports.find((result) => result.request.start_date === '2000-12-06')
        ?.text,
    ).toContain('at most 31 days')
    const requestsBeforeRead = embeddings.requests()
    expect(
      (
        await backend<SimilarSearches>('get_similar_searches', {
          query: NOT_INDEXED,
          wordings: [],
        })
      ).state,
    ).toBe('notIndexed')
    await visit(
      page,
      `/#/insights/search/${encodeURIComponent(NOT_INDEXED)}?range=all`,
    )
    await expect(
      page.getByText('This search hasn’t been indexed yet.', { exact: true }),
    ).toBeVisible()

    await visit(
      page,
      `/#/insights/search/${encodeURIComponent(SUBJECT_A[0])}?range=all`,
    )
    const similar = page
      .getByRole('heading', { name: 'Similar searches', exact: true })
      .locator('xpath=ancestor::section[1]')
    await expect(similar.locator('[data-similar-query]')).toHaveCount(2)
    for (const query of SUBJECT_A.slice(1))
      await expect(
        similar.getByRole('link', { name: `Show searches for “${query}”` }),
      ).toBeVisible()
    for (const query of SUBJECT_B)
      await expect(
        similar.locator(`[data-similar-query="${query}"]`),
      ).toHaveCount(0)
    const rows = await backend<SimilarSearches>('get_similar_searches', {
      query: SUBJECT_A[0],
      wordings: [],
    })
    expect(rows.state).toBe('ready')
    if (rows.state === 'ready')
      expect(rows.items.map((row) => [row.query, row.searches]).sort()).toEqual(
        SUBJECT_A.slice(1)
          .map((query) => [query, 1])
          .sort(),
      )
    await testInfo.attach('similar search rows', {
      body: JSON.stringify(rows, null, 2),
      contentType: 'application/json',
    })
    await testInfo.attach('similar search drill-in', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    await similar
      .getByRole('link', { name: `Show searches for “${SUBJECT_A[1]}”` })
      .click()
    await expect(
      page.getByRole('heading', { name: `“${SUBJECT_A[1]}”`, exact: true }),
    ).toBeVisible()
    expect(embeddings.requests()).toBe(requestsBeforeRead)
    await visit(page, '/#/settings/ai')
    await page
      .getByRole('button', { name: 'Remove', exact: true })
      .first()
      .click()
    const remove = page.getByRole('alertdialog', {
      name: 'Remove E2E search group embeddings?',
    })
    await remove.getByRole('button', { name: 'Remove', exact: true }).click()
    await expect(remove).toBeHidden()
    await visit(
      page,
      `/#/insights/search/${encodeURIComponent(SUBJECT_A[0])}?range=all`,
    )
    await expect(
      page.getByRole('heading', { name: 'Similar searches', exact: true }),
    ).toHaveCount(0)
    completed = true
  } finally {
    await testInfo.attach('search groups and exact embedding inputs', {
      body: JSON.stringify(
        {
          reports,
          searchesSent: embeddings.inputs.filter((text) =>
            EMBEDDED_QUERIES.includes(text),
          ),
          visits,
        },
        null,
        2,
      ),
      contentType: 'application/json',
    })
    const restore = async () => {
      for (const filename of imported.reverse())
        await undoRelatedFile(page, filename)
      const current = await config()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    }
    try {
      await restore().catch(async (error: unknown) => {
        if (completed) throw error
        await testInfo.attach('search group cleanup error', {
          body: String(error),
        })
      })
    } finally {
      await Promise.all([model.close(), embeddings.close()])
    }
  }
}
