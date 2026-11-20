/**
 * Searching saved conversations by meaning in Records (`agent-history.md`
 * §3.4 and §7 "AI access"), on the conversations the agent projects
 * backed up, with an embeddings service of the user's own.
 *
 * Proves:
 * - Turning on Semantic search in Settings › AI and then "Build a semantic
 *   search index" in Settings › Agent conversations builds an index of the
 *   conversations with the chosen service: the backend says it serves a
 *   current generation, Settings says Ready, and the service was sent the
 *   conversations' passages.
 * - A search that no record contains word for word (Records' full-text
 *   search says nothing mentions it, on screen and in the backend) finds,
 *   in semantic mode reading only what was said (yours and the agent's),
 *   the prompt in the middle of the 112-turn conversation first, and
 *   nothing is lit. The backend answers in pages of 100, more than one
 *   here. Records shows the first page and says more load as you scroll;
 *   scrolled to the end it loads page after page until the line says
 *   that's all: more than 100 passages, as many as the backend's pages
 *   add up to when read with their cursors, in the backend's order across
 *   the page boundaries, none twice. Opening the first puts the
 *   conversation and `at=<ordinal>:<source>` in the address, with no find
 *   words, and the reader shows that prompt, marked, far below the first
 *   turn.
 * - With the switch off again, the backend refuses semantic searches with
 *   `agent-conversations-access-disabled`, and Records sends the search
 *   and shows that refusal: what to turn on and where, with a link that
 *   goes there and a way back to full text, which still answers.
 *
 * Assumptions:
 * - The service is `support/fake-embeddings.ts`: a bag of words, so a
 *   search is close to a passage only by sharing exact words. The search
 *   reuses the prompt's words in another order, with its one made-up word,
 *   plus "slower" and "a", which that prompt lacks; the in-app model would
 *   also match other forms of the words, but the test model has none of
 *   the fixture's vocabulary, and builtin-model.spec leaves the app without
 *   its address. The service's vectors are checked first: the prompt must
 *   lead every other text it was sent by a clear margin, so a hash
 *   collision fails there, by name, not as a wrong first result.
 * - The backend streams equal scores in a fixed order: two reads of the
 *   same search give the same order (checked before the screen is).
 * - There is no minimum score, so every indexed passage of yours and the
 *   agent's is a result: the large conversation alone has 112 prompts and
 *   two agent texts a turn, at least 224, so the roles are not widened to
 *   get past one page. The real total comes from the backend.
 * - AI settings and this computer's Agent switches are as builtin-model.spec
 *   left them; this spec puts both back through the backend when it ends.
 *   The index it built stays in the archive, unused while the switch is off.
 */
import { expect, test, type Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { addEmbeddingsService } from './support/ai-service'
import {
  agentChat,
  hashParam,
  openHistory,
  reader,
  readerTitle,
  savedChat,
  transcript,
} from './support/agent-chats'
import {
  passageKey,
  semanticStatus,
  wireSearch,
  wireSemantic,
} from './support/agent-search'
import { unlockIfLocked } from './support/app'
import {
  fakeEmbeddingVector,
  startFakeEmbeddings,
  type FakeEmbeddings,
} from './support/fake-embeddings'
import { backend, fixture } from './support/fixture'

test.describe.configure({ mode: 'serial' })

/** Wide, so few of the fixture's words share a slot with the search's. */
const DIMENSIONS = 1024
const MODEL = 'e2e-embed'
const SERVICE = 'E2E conversation embeddings'
const ROLES = ['user', 'agent', 'tool', 'thinking']
/**
 * What the semantic search reads: tool output is left out, as a person
 * looking for something said would; its numbers share the fake's slots
 * with the search's words and would add thousands of passages.
 */
const SAID = ['user', 'agent']
const ACTIVE = ['running', 'paused', 'cancelling']

interface Config {
  ai: Record<string, unknown>
}
const loadConfig = async () =>
  (await backend<{ config: Config }>('app_snapshot')).config
const queueStatus = () =>
  backend<{ running: number; semanticIndex: { chosenComplete: boolean } }>(
    'load_ai_queue_status',
  )
const viewerPreferences = () =>
  backend<{ buildSemanticIndex: boolean }>('get_agent_viewer_preferences', {
    request: {},
  })

let service: FakeEmbeddings | null = null
/** What this spec changes, as it found it. */
let before: { ai: Config['ai']; buildSemanticIndex: boolean } | null = null
/** The texts the conversations' build sent the service, nothing else. */
let documents: string[] = []

const number = (n: number) => new Intl.NumberFormat('en-US').format(n)

/** The request Records sends for `query` with no filters (full text). */
async function request(query: string, roles = ROLES) {
  const { shown } = await backend<{ shown: string }>(
    'get_insights_time_zone',
    {},
  )
  return {
    query,
    roles,
    filters: { harnesses: [], flags: [], range: 'all', timeZone: shown },
  }
}

const searchBox = (page: Page) =>
  page.getByRole('searchbox', { name: 'Search conversations' })
const feed = (page: Page) => page.getByRole('feed', { name: 'Search results' })
const passages = (page: Page) => feed(page).locator('[data-passage]')
/** The line under the choices (what is said aloud is a copy of it). */
const status = (page: Page) => page.locator('[data-search-status]')
const buildSwitch = (page: Page) =>
  page.getByRole('switch', { name: 'Build a semantic search index' })

/** The large conversation's middle prompt, its made-up word, and a search for it. */
function scenario() {
  const large = fixture().agents.scenes.large
  const word = large.middleWord
  return {
    large,
    word,
    query: `slower p95 with a warm cache ${word}`,
  }
}

const dot = (a: number[], b: number[]) =>
  a.reduce((sum, value, index) => sum + value * b[index], 0)

const scrollHeight = (page: Page) =>
  feed(page).evaluate((element) => element.scrollHeight)

/**
 * Every passage the feed draws, read top to bottom by its place in the
 * list (`data-index`), scrolling a screen at a time. At the bottom it waits
 * for the next page to come in, until the status line says `end`; a row
 * never drawn fails here, by name.
 */
async function drawnOrder(page: Page, end: string) {
  const seen = new Map<number, string>()
  await feed(page).evaluate((element) => {
    element.scrollTop = 0
  })
  for (let step = 0; ; step++) {
    expect(step, 'the list reaches its end').toBeLessThan(20_000)
    await page.evaluate(
      () =>
        new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(resolve)),
        ),
    )
    // Read before the rows: a last page that lands after them would be missed.
    const whole = (await status(page).textContent()) === end
    const rows = await feed(page)
      .locator('[data-passage]')
      .evaluateAll((elements) =>
        elements.map(
          (element) =>
            [
              Number(
                element.closest('[data-index]')?.getAttribute('data-index'),
              ),
              element.getAttribute('data-passage') ?? '',
            ] as const,
        ),
      )
    for (const [index, key] of rows) seen.set(index, key)
    // The height read with the scroll, so a page landing between them is
    // not waited for again.
    const { moved, height } = await feed(page).evaluate((element) => {
      const from = element.scrollTop
      element.scrollTop = from + element.clientHeight
      return {
        moved: element.scrollTop !== from,
        height: element.scrollHeight,
      }
    })
    if (moved) continue
    if (whole) break
    // At the bottom before the last page: the next one is on its way.
    await expect
      .poll(
        async () =>
          (await status(page).textContent()) === end ||
          (await scrollHeight(page)) > height,
        { message: 'the next page comes in at the bottom', timeout: 60_000 },
      )
      .toBe(true)
  }
  const indexes = [...seen.keys()].sort((a, b) => a - b)
  expect(indexes.at(-1), 'no row skipped').toBe(indexes.length - 1)
  return indexes.map((index) => seen.get(index) ?? '')
}

test.beforeAll(async () => {
  service = await startFakeEmbeddings({ dimensions: DIMENSIONS })
})

test.afterAll(async ({ browser }, info) => {
  info.setTimeout(240_000)
  try {
    if (!before) return
    const page = await browser.newPage({
      baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
    })
    try {
      await page.goto('/#/settings/agent')
      await unlockIfLocked(page)
      // No build of this spec's left running against a service about to close.
      const { job } = await semanticStatus()
      if (job && ACTIVE.includes(job.state) && job.state !== 'cancelling')
        await backend('control_agent_semantic_index', {
          request: { action: 'cancel', jobId: job.jobId },
        })
      await expect
        .poll(async () => (await semanticStatus()).job?.state ?? 'none', {
          timeout: 120_000,
        })
        .not.toMatch(/^(running|paused|cancelling)$/)
      await backend('set_agent_viewer_preferences', {
        request: { buildSemanticIndex: before.buildSemanticIndex },
      })
      await expect
        .poll(async () => (await queueStatus()).running, { timeout: 120_000 })
        .toBe(0)
      const current = await loadConfig()
      await backend('save_config', { config: { ...current, ai: before.ai } })
    } finally {
      await page.close()
    }
  } finally {
    await service?.close()
  }
})

test('the switches build an index of the conversations with the chosen service', async ({
  page,
}) => {
  test.setTimeout(480_000)
  await page.goto('/#/settings/ai')
  await unlockIfLocked(page)
  before = {
    ai: (await loadConfig()).ai,
    buildSemanticIndex: (await viewerPreferences()).buildSemanticIndex,
  }
  expect(
    before.buildSemanticIndex,
    'the switch starts off, as on a new installation, so this build is the first',
  ).toBe(false)
  // Index work another spec left must not be the one waited for below.
  await expect
    .poll(async () => (await queueStatus()).running, { timeout: 120_000 })
    .toBe(0)

  await addEmbeddingsService(page, service!.url, MODEL, SERVICE)
  const semantic = page.getByRole('switch', { name: 'Semantic search' })
  if (!(await semantic.isChecked())) await semantic.click()
  else
    await page.getByRole('button', { name: 'Build index', exact: true }).click()
  await expect(semantic).toBeChecked()
  // Browsing history's index first: what it sends is not the conversations'.
  await expect
    .poll(async () => (await queueStatus()).semanticIndex.chosenComplete, {
      timeout: 240_000,
    })
    .toBe(true)
  await expect
    .poll(async () => (await queueStatus()).running, { timeout: 120_000 })
    .toBe(0)
  const from = service!.inputs.length

  await page.goto('/#/settings/agent')
  await unlockIfLocked(page)
  await expect(buildSwitch(page)).toBeVisible()
  if (!(await buildSwitch(page).isChecked())) await buildSwitch(page).click()
  await expect(buildSwitch(page)).toBeChecked()
  await expect
    .poll(
      async () => {
        const now = await semanticStatus()
        return (
          now.enabled &&
          now.serving !== null &&
          now.stale === false &&
          !ACTIVE.includes(now.job?.state ?? '')
        )
      },
      { timeout: 240_000, intervals: [500, 1_000, 2_000] },
    )
    .toBe(true)
  const { serving } = await semanticStatus()
  expect(serving!.model).toBe(MODEL)
  await expect(
    page.getByRole('status', { name: 'Semantic search index' }),
  ).toContainText('Ready')

  documents = service!.inputs.slice(from)
  const { word } = scenario()
  expect(
    documents.filter((text) => text.includes(word)),
    'the middle prompt was sent once, and nothing else has its word',
  ).toHaveLength(1)
})

test('a search no record says word for word finds the prompt by meaning, in the backend’s order', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  const { large, word, query } = scenario()
  const chat = await savedChat(agentChat(large.conversation))
  const [target] = documents.filter((text) => text.includes(word))
  for (const shared of ['p95', 'with', 'warm', 'cache'])
    expect(target, `the prompt says “${shared}”`).toContain(shared)
  for (const missing of ['slower', ' a '])
    expect(target, `the prompt lacks “${missing.trim()}”`).not.toContain(
      missing,
    )

  // The service's own vectors put the prompt first, well ahead of the rest.
  const searched = fakeEmbeddingVector(query, DIMENSIONS)
  const ranked = documents
    .map((text) => ({
      text,
      score: dot(searched, fakeEmbeddingVector(text, DIMENSIONS)),
    }))
    .sort((a, b) => b.score - a.score)
  expect(ranked[0].text).toBe(target)
  expect(
    ranked[0].score - ranked[1].score,
    'the margin survives int8 vectors',
  ).toBeGreaterThan(0.02)

  // Full text finds nothing; by meaning the prompt comes first, and the
  // order is the same every time.
  const words = await wireSearch(await request(query))
  expect(words.conversations).toBe(0)
  const expected = await wireSemantic(await request(query, SAID))
  const again = await wireSemantic(await request(query, SAID))
  const keys = expected.passages.map(passageKey)
  expect(again.passages.map(passageKey)).toEqual(keys)
  expect(expected.stale).toBe(false)
  expect(expected.hits).toBe(expected.passages.length)
  // Pages of 100, only the last one short, and more than one of them.
  expect(expected.pageSize).toBe(100)
  expect(expected.hits, 'more passages than a page holds').toBeGreaterThan(
    expected.pageSize,
  )
  expect(
    expected.pages.slice(0, -1),
    `pages of ${expected.pages.join(', ')}`,
  ).toEqual(expected.pages.slice(0, -1).map(() => expected.pageSize))
  expect(expected.pages.at(-1)).toBeLessThanOrEqual(expected.pageSize)
  expect(new Set(keys).size, 'no passage on two pages').toBe(keys.length)
  const [first] = expected.passages
  expect(first.id).toBe(chat.id)
  expect(first.address.conversationId, 'a record of its own').toBe(chat.id)
  expect(first.role).toBe('user')
  expect(first.text).toContain(`(${word})`)

  // On screen: full text first, as typed, then the mode.
  await openHistory(page)
  await searchBox(page).fill(query)
  await expect.poll(() => hashParam(page, 'q')).toBe(query)
  await expect(
    page.getByText(`No conversation mentions “${query}”.`),
  ).toBeVisible({ timeout: 60_000 })
  await page.getByRole('radio', { name: 'Semantic', exact: true }).click()
  await expect.poll(() => hashParam(page, 'mode')).toBe('semantic')
  await expect(searchBox(page)).toHaveAttribute(
    'placeholder',
    'Describe what you’re looking for…',
  )
  // Only what was said: the running search stops and the narrower one runs.
  const kinds = page.getByRole('group', { name: 'What to search' })
  await kinds.getByRole('button', { name: 'Tool output' }).click()
  await kinds.getByRole('button', { name: 'Thinking' }).click()
  await expect.poll(() => hashParam(page, 'roles')).toBe(SAID.join(','))
  // The first page, and the list says there is more.
  await expect(status(page)).toHaveText(
    `${number(expected.pageSize)} passages, closest first · more load as you scroll`,
    { timeout: 60_000 },
  )
  await expect(
    page.getByText(
      new RegExp(`^Semantic search with ${SERVICE} · ${MODEL}, built `),
    ),
  ).toBeVisible()

  const top = passages(page).first()
  await expect(top).toHaveAttribute('data-passage', passageKey(first))
  await expect(top).toContainText(chat.title ?? '')
  await expect(top).toContainText('You')
  await expect(top).toContainText(`(${word})`)
  await expect(feed(page).locator('mark')).toHaveCount(0)
  await testInfo.attach('the prompt found by meaning', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Scrolled to the end, page after page: every passage the backend's pages
  // hold, in their order across the page boundaries, none twice, and the
  // line says that's all.
  const end = `${number(expected.hits)} passages, closest first · that’s all`
  const order = await drawnOrder(page, end)
  expect(order.length, 'pages after the first loaded').toBeGreaterThan(
    expected.pageSize,
  )
  expect(order, 'as many rows as the backend’s pages hold').toHaveLength(
    expected.hits,
  )
  expect(new Set(order).size, 'no passage twice').toBe(order.length)
  expect(order).toEqual(keys)
  await expect(status(page)).toHaveText(end)
  await expect(feed(page)).toHaveAttribute('aria-busy', 'false')
  await testInfo.attach('every page loaded', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Opening it: the reader at that prompt, with no words to find.
  await feed(page).evaluate((element) => {
    element.scrollTop = 0
  })
  await top.getByRole('button', { name: chat.title ?? undefined }).click()
  await expect.poll(() => hashParam(page, 'chat')).toBe(chat.id)
  expect(hashParam(page, 'at')).toBe(
    `${first.address.ordinal}:${first.address.sourceId}`,
  )
  expect(hashParam(page, 'find')).toBeNull()
  // The reader marks the record it opened at for a moment.
  await expect(
    transcript(page).locator('[data-opened-at="true"]'),
  ).toContainText(`(${word})`, { timeout: 30_000 })
  await expect(readerTitle(page)).toHaveText(chat.title ?? '')
  await expect(transcript(page).getByText(`(${word})`)).toBeInViewport({
    timeout: 30_000,
  })
  // The first turn is far above: the reader did not stay at the start.
  await expect(transcript(page).getByText(`(${large.firstWord})`)).toHaveCount(
    0,
  )
  await expect(top).toHaveAttribute('aria-current', 'true')
  await expect(reader(page)).toBeVisible()
  await testInfo.attach('the reader at the passage’s record', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('with the switch off, semantic mode says what to turn on and where', async ({
  page,
}, testInfo) => {
  const { query } = scenario()
  await page.goto('/#/settings/agent')
  await unlockIfLocked(page)
  await expect(buildSwitch(page)).toBeChecked()
  await buildSwitch(page).click()
  await expect(buildSwitch(page)).not.toBeChecked()
  await expect.poll(async () => (await semanticStatus()).enabled).toBe(false)
  await expect(
    wireSemantic(await request(query)),
    'the backend refuses it too',
  ).rejects.toThrow(/agent-conversations-access-disabled/)

  await openHistory(page, `?q=${encodeURIComponent(query)}&mode=semantic`)
  // The status can't say which switch is off; the search's refusal does.
  const blocked = page.locator(
    '[data-semantic-block="agent-conversations-access-disabled"]',
  )
  await expect(blocked).toContainText('Semantic search of conversations is off')
  await expect(blocked).toContainText(
    'Turn on “Build a semantic search index” in Settings › Agent conversations.',
  )
  await expect(feed(page)).toHaveCount(0)
  await testInfo.attach('semantic mode with the switch off', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // The link goes to the switch.
  await blocked
    .getByRole('link', { name: 'Settings › Agent conversations' })
    .click()
  await expect(page).toHaveURL(/#\/settings\/agent$/)
  await expect(buildSwitch(page)).not.toBeChecked()

  // Back in Records, full text still answers, from the same search.
  await page.goBack()
  await expect(blocked).toBeVisible()
  await blocked
    .getByRole('button', { name: 'Search the full text instead' })
    .click()
  await expect.poll(() => hashParam(page, 'mode')).toBeNull()
  expect(hashParam(page, 'q')).toBe(query)
  await expect(
    page.getByText(`No conversation mentions “${query}”.`),
  ).toBeVisible({ timeout: 60_000 })
  await expect(
    page.getByRole('radio', { name: 'Full text', exact: true }),
  ).toHaveAttribute('aria-checked', 'true')
})
