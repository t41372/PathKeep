/**
 * Searching the saved conversations from History (`agent-history.md`
 * §3.4), on the archive the agent projects left.
 *
 * Proves, against what the backend's own search returns for the same
 * request (`search_agent_conversations`, read to its last frame):
 * - `/` puts the cursor in the search box; a word found in many
 *   conversations streams every one of them, never cut short: the line
 *   says exactly how many conversations and matches and how many were
 *   searched, the first card is the backend's first and scrolling the
 *   results reaches the backend's last card; the search is a history
 *   entry of its own, so Back returns to the list;
 * - a regular expression finds the 13 runs of the long conversation that
 *   are numbered 100 to 112, in one card with three snippets and "10
 *   more"; with "Agent's words" off nothing matches, and the empty state
 *   says what was searched; a regular expression that does not compile
 *   says so instead of "no results";
 * - a word only a subagent wrote is found in its parent's card, marked as
 *   the subagent's; opening that snippet opens the conversation with the
 *   subagent in its side sheet, find on that word, and the word lit;
 * - a word in one prompt of the long conversation, in the middle of its
 *   112 turns, opens the reader at that prompt with the word lit;
 * - ⌘F with no conversation open goes to the search box; Esc there clears
 *   the search and the list comes back.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  agentChat,
  hashParam,
  listRows,
  openHistory,
  reader,
  savedChat,
  transcript,
} from './support/agent-chats'
import { findHighlights, wireFind, wireSearch } from './support/agent-search'
import { backend, fixture } from './support/fixture'

const ROLES = ['user', 'agent', 'tool', 'thinking']
const number = (n: number) => new Intl.NumberFormat('en-US').format(n)
const plural = (n: number, one: string, other: string) =>
  `${number(n)} ${n === 1 ? one : other}`

async function zone() {
  return (await backend<{ shown: string }>('get_insights_time_zone', {})).shown
}

/** The request History sends for `query` with no filters. */
async function request(query: string, roles = ROLES) {
  return {
    query,
    roles,
    filters: { harnesses: [], flags: [], range: 'all', timeZone: await zone() },
  }
}

const searchBox = (page: Page) =>
  page.getByRole('searchbox', { name: 'Search conversations' })
const results = (page: Page) =>
  page.getByRole('feed', { name: 'Search results' })
const cards = (page: Page) => results(page).locator('[data-result-id]')
const card = (page: Page, id: string) =>
  results(page).locator(`[data-result-id="${id}"]`)
/** The line under the box (what is said aloud is a copy of it). */
const status = (page: Page) => page.locator('[data-search-status]')
const findBar = (page: Page) =>
  page.getByRole('search', { name: 'Find in this conversation' })

/** The words under the box once the search has finished. */
function doneLine(found: Awaited<ReturnType<typeof wireSearch>>) {
  const searched = found.filtered
    ? plural(found.searched, 'filtered conversation', 'filtered conversations')
    : plural(found.searched, 'conversation', 'conversations')
  return `${plural(found.conversations, 'conversation', 'conversations')}, ${plural(found.hits, 'match', 'matches')} in all · searched ${searched}`
}

test('a word in many conversations streams to the end, as the backend counts it', async ({
  page,
}, testInfo) => {
  const query = 'the'
  const expected = await wireSearch(await request(query))
  expect(
    expected.conversations,
    'a word most conversations use',
  ).toBeGreaterThan(10)

  await openHistory(page)
  await page.locator('body').press('/')
  await expect(searchBox(page)).toBeFocused()
  await searchBox(page).fill(query)
  await expect.poll(() => hashParam(page, 'q')).toBe(query)
  await expect(status(page)).toHaveText(doneLine(expected), {
    timeout: 60_000,
  })
  await expect(cards(page).first()).toHaveAttribute(
    'data-result-id',
    expected.order[0],
  )
  await expect(cards(page).first()).toContainText(
    plural(expected.cards.get(expected.order[0])!.hits, 'match', 'matches'),
  )
  await testInfo.attach('every conversation that says “the”', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Every card is there: the last one the backend sent is the last drawn.
  await expect(async () => {
    await results(page).evaluate((element) => {
      element.scrollTop = element.scrollHeight
    })
    await expect(cards(page).last()).toHaveAttribute(
      'data-result-id',
      expected.order.at(-1)!,
      { timeout: 1_000 },
    )
  }).toPass({ timeout: 30_000 })

  // The search was its own history entry.
  await page.goBack()
  await expect.poll(() => hashParam(page, 'q')).toBeNull()
  await expect(listRows(page).first()).toBeVisible()
  await expect(searchBox(page)).toHaveValue('')
})

test('a regular expression, the kinds of text it reads, and one that does not compile', async ({
  page,
}, testInfo) => {
  const large = fixture().agents.scenes.large
  const chat = await savedChat(agentChat(large.conversation))
  const query = '/running run 1\\d\\d now/'
  const expected = await wireSearch(await request(query))
  expect(expected.order).toEqual([chat.id])
  expect(expected.hits, 'runs 100 to 112').toBe(13)

  await openHistory(page, `?q=${encodeURIComponent(query)}`)
  await expect(status(page)).toHaveText(doneLine(expected), {
    timeout: 60_000,
  })
  await expect(cards(page)).toHaveCount(1)
  const only = card(page, chat.id)
  await expect(only).toContainText('13 matches')
  await expect(only.locator('li')).toHaveCount(3)
  await expect(only.locator('li mark').first()).toHaveText(
    /^Running run 1\d\d now$/i,
  )
  await expect(only).toContainText(
    '10 more in this conversation; open it and press Enter to step through',
  )
  await testInfo.attach('a regular expression', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Without the agent's words nothing matches, and it says what it read.
  const roles = page.getByRole('group', { name: 'What to search' })
  await roles.getByRole('button', { name: 'Agent’s words' }).click()
  await expect.poll(() => hashParam(page, 'roles')).toBe('user,tool,thinking')
  const without = await wireSearch(
    await request(query, ['user', 'tool', 'thinking']),
  )
  expect(without.conversations).toBe(0)
  await expect(
    page.getByText(`No conversation mentions “${query}”.`),
  ).toBeVisible({ timeout: 60_000 })
  const kinds = new Intl.ListFormat('en', { type: 'conjunction' }).format([
    'what you said',
    'tool output',
    'thinking',
  ])
  await expect(
    page.getByText(
      `Searched ${kinds} in the ${without.searched} conversations that match the filters.`,
    ),
  ).toBeVisible()
  await roles.getByRole('button', { name: 'Agent’s words' }).click()
  await expect.poll(() => hashParam(page, 'roles')).toBeNull()

  // One that does not compile is not "no results".
  await searchBox(page).fill('/run (/')
  await searchBox(page).press('Enter')
  await expect(
    page.getByRole('alert').getByText('This regular expression is wrong'),
  ).toBeVisible({ timeout: 30_000 })
  await expect(page.getByText(/^No conversation mentions/)).toHaveCount(0)
})

test('a word only a subagent wrote opens in the subagent, lit', async ({
  page,
}, testInfo) => {
  const parent = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(parent)
  const sub = parent.subagents[2]
  const word = sub.words[0]
  const expected = await wireSearch(await request(word))
  expect(expected.order).toEqual([chat.id])
  const occurrences = await wireFind(chat.id, word)

  await openHistory(page, `?q=${encodeURIComponent(word)}`)
  const only = card(page, chat.id)
  await expect(only).toBeVisible({ timeout: 60_000 })
  const snippet = only.locator('li').filter({ hasText: 'subagent' }).first()
  await expect(snippet.locator('mark')).toHaveText(new RegExp(`^${word}$`, 'i'))
  await snippet.getByRole('button').click()

  await expect.poll(() => hashParam(page, 'chat')).toBe(chat.id)
  expect(hashParam(page, 'find')).toBe(word)
  expect(hashParam(page, 'at')).toMatch(/^\d+:/)
  const sheet = page.getByRole('dialog')
  await expect(sheet.getByRole('heading', { name: sub.title })).toBeVisible()
  await expect(findBar(page)).toContainText(
    new RegExp(`\\d+ / ${occurrences}$`),
  )
  await expect
    .poll(async () => (await findHighlights(page)).current?.toLowerCase())
    .toBe(word.toLowerCase())
  await testInfo.attach('the subagent, opened at its word', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('a word in the middle of the long conversation opens the reader there', async ({
  page,
}) => {
  const large = fixture().agents.scenes.large
  const chat = await savedChat(agentChat(large.conversation))
  const word = large.middleWord
  const occurrences = await wireFind(chat.id, word)
  await openHistory(page, `?q=${encodeURIComponent(word)}`)
  const only = card(page, chat.id)
  await expect(only).toBeVisible({ timeout: 60_000 })
  await only.locator('li').first().getByRole('button').click()
  await expect.poll(() => hashParam(page, 'chat')).toBe(chat.id)
  await expect(findBar(page)).toContainText(
    new RegExp(`\\d+ / ${occurrences}$`),
  )
  await expect(transcript(page).getByText(`(${word})`)).toBeInViewport()
  await expect
    .poll(async () => (await findHighlights(page)).current?.toLowerCase())
    .toBe(word.toLowerCase())
  // The first turn is far above: the reader did not read from the start.
  await expect(transcript(page).getByText(`(${large.firstWord})`)).toHaveCount(
    0,
  )
  await expect(reader(page)).toBeVisible()
})

test('⌘F goes to the search box with no conversation open; Esc clears it', async ({
  page,
}) => {
  await openHistory(page)
  await page.locator('body').press('ControlOrMeta+f')
  await expect(searchBox(page)).toBeFocused()
  await searchBox(page).fill('benchmark')
  await expect.poll(() => hashParam(page, 'q')).toBe('benchmark')
  await expect(cards(page).first()).toBeVisible({ timeout: 60_000 })
  await searchBox(page).press('Escape')
  await expect.poll(() => hashParam(page, 'q')).toBeNull()
  await expect(listRows(page).first()).toBeVisible()
})
