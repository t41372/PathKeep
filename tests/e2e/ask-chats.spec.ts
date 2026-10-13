/**
 * Ask's saved chats, and how far one answer can run, against a scripted
 * OpenAI-compatible service (`support/fake-model.ts`).
 *
 * Proves:
 * - A follow-up goes into the same chat: after a reload the list holds it
 *   once, under its first question, with both questions and both answers in
 *   the order they were asked; the chat keeps the time it was created, and the
 *   model was sent the first question and answer with the second.
 * - A deleted chat whose answer cited pages is gone from the list, also after
 *   a reload, and from the archive: opening it finds nothing and deleting it
 *   again deletes nothing. A new chat shows none of its pages.
 * - A chat comes back as it was shown: the question, the model's thinking,
 *   the search it ran and what that returned, the answer and the pages it
 *   used, in the same order and words, under the question as its title.
 * - A model that never stops calling tools is stopped at the step limit
 *   (`DEFAULT_MAX_ITERATIONS` in `agent_harness.rs`): it is asked once more,
 *   without tools, and that answer is shown with a note saying why it
 *   stopped. The service gets exactly the step limit plus that last request.
 * - "When did I first…": a search sorted oldest first returns to the model
 *   the earliest visit of a word found across several years of the fixture,
 *   not a recent one (a real answer once gave the latest month instead).
 */
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { addService } from './support/ai-service'
import {
  chatRequests,
  lastUserText,
  offersTools,
  startFakeModel,
  textOf,
  toolResults,
  type FakeRequest,
  type Script,
} from './support/fake-model'
import { archivedVisits, backend, expectCount } from './support/fixture'

const KEY = 'e2e-chats-key'

/** The fixture's GitHub issue about spawn_blocking (the rust topic). */
const ISSUE_TITLE = 'spawn_blocking pool exhaustion · tokio-rs/tokio'

interface SavedChat {
  id: string
  title: string
  createdAt: string
  updatedAt: string
  messageCount: number
}

interface SavedMessage {
  role: string
  content: string
  citations?: { url: string }[] | null
}

/** Every saved chat, through all of the list's pages. */
async function savedChats() {
  const chats: SavedChat[] = []
  let cursor: string | null = null
  do {
    const listPage: {
      conversations: SavedChat[]
      nextCursor: string | null
    } = await backend('list_ai_conversations', {
      request: { limit: 100, cursor },
    })
    chats.push(...listPage.conversations)
    cursor = listPage.nextCursor
  } while (cursor)
  return chats
}

/**
 * The saved chat titled `title`, once it holds `messages` messages. Ask saves
 * a chat after each answer, without anything on screen saying so.
 */
async function savedChat(title: string, messages: number) {
  let found: SavedChat[] = []
  await expect
    .poll(
      async () => {
        found = (await savedChats()).filter((chat) => chat.title === title)
        return found.map((chat) => chat.messageCount)
      },
      { timeout: 30_000 },
    )
    .toEqual([messages])
  return found[0]
}

async function startService(page: Page, name: string, script: Script) {
  const model = await startFakeModel({ apiKey: KEY, script })
  await addService(page, { name, address: model.url, key: KEY })
  await page.goto('/#/ask')
  return model
}

/** Sends `question` and waits until its answer has finished. */
async function ask(page: Page, question: string, answer: string) {
  const composer = page.getByRole('textbox', { name: 'Message' })
  await composer.fill(question)
  await composer.press('Enter')
  await expect(chatColumn(page).getByText(answer)).toBeVisible({
    timeout: 60_000,
  })
  await expect(
    page.getByRole('button', { name: 'Send', exact: true }),
  ).toBeVisible()
}

const conversationList = (page: Page) =>
  page.getByRole('complementary', { name: 'Conversations' })

/** A saved chat's row in the list (its name is the title and the time). */
const listed = (page: Page, title: string) =>
  conversationList(page).getByRole('button', { name: title })

/** The open chat and its message box: the column beside the list. */
const chatColumn = (page: Page) => page.getByRole('main').locator('aside + div')

/**
 * What the open chat shows, top to bottom, with each thinking block opened.
 * The pages an answer used are read apart, as a set, and the token count is
 * left out: see the notes where they are compared.
 */
async function readChat(page: Page) {
  const column = chatColumn(page)
  for (const trigger of await column
    .getByRole('button', { name: /^(Thinking|Searched the archive)/ })
    .all()) {
    if ((await trigger.getAttribute('aria-expanded')) !== 'true')
      await trigger.click()
  }
  // Each page's tooltip: its title and the day of the visit cited.
  const shownPages = await column
    .getByRole('group', { name: 'Pages used' })
    .getByRole('link')
    .evaluateAll((links) => links.map((link) => link.getAttribute('title')))
  const pages = [...shownPages].sort()
  const { text, tokens } = await column.evaluate((root: HTMLElement) => {
    const tokenCount = /^\d+ in · \d+ out$/
    const hide = [
      ...root.querySelectorAll<HTMLElement>('[aria-label="Pages used"]'),
      ...[...root.querySelectorAll<HTMLElement>('span')].filter((span) =>
        tokenCount.test(span.textContent ?? ''),
      ),
    ]
    const tokens = hide
      .filter((element) => tokenCount.test(element.textContent ?? ''))
      .map((element) => element.textContent)
    for (const element of hide) element.style.display = 'none'
    const text = root.innerText
    for (const element of hide) element.style.display = ''
    return { text, tokens }
  })
  const openInHistory = await column
    .getByRole('link', { name: 'Open in History' })
    .evaluateAll((links) => links.map((link) => link.getAttribute('href')))
  return { text, pages, shownPages, tokens, openInHistory }
}

/** Asserts that `parts` appear in `text` in this order. */
function expectInOrder(text: string, parts: string[]) {
  const positions = parts.map((part) => text.indexOf(part))
  for (const [index, part] of parts.entries())
    expect(positions[index], `“${part}” is shown`).toBeGreaterThanOrEqual(0)
  expect(positions, `${parts.join(' → ')}, in this order`).toEqual(
    [...positions].sort((a, b) => a - b),
  )
}

/** The user and assistant text the model was sent, in order. */
function conversationSent(body: FakeRequest['body']) {
  return (body.messages ?? [])
    .filter(({ role }) => role === 'user' || role === 'assistant')
    .map(({ content }) => textOf(content))
    .filter(Boolean)
}

/** A `u32` constant from the Rust source, so the spec follows the code. */
function rustConstant(file: string, name: string) {
  const source = readFileSync(path.resolve(file), 'utf8')
  const value = new RegExp(`const ${name}: u32 = ([\\d_]+);`).exec(source)?.[1]
  if (!value) throw new Error(`${name} is not in ${file}`)
  return Number(value.replaceAll('_', ''))
}

/** `YYYY-MM-DD HH:MM` in this computer's time zone, as the search tool writes visit times. */
function localMinute(at: number) {
  const time = new Date(at)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${time.getFullYear()}-${pad(time.getMonth() + 1)}-${pad(time.getDate())} ${pad(time.getHours())}:${pad(time.getMinutes())}`
}

/** Searches for `query` when asked a question, then answers from `answers`. */
function searchThenAnswer(
  query: object,
  answers: Record<string, string>,
): Script {
  return (body) => {
    const question = lastUserText(body)
    if (body.messages?.at(-1)?.role === 'tool' || !offersTools(body))
      return { content: answers[question] ?? `No answer for “${question}”.` }
    return { toolCalls: [{ name: 'search_history', arguments: query }] }
  }
}

test('a follow-up stays in its chat, saved in order under the first question', async ({
  page,
}) => {
  const first = 'Which page explained spawn_blocking pool exhaustion?'
  const second = 'And which project was that issue filed in?'
  const answers = {
    [first]: 'It was a GitHub issue about running out of blocking threads.',
    [second]: 'It was filed in tokio-rs/tokio.',
  }
  const model = await startService(
    page,
    'E2E follow-up',
    searchThenAnswer({ query: 'spawn_blocking' }, answers),
  )
  try {
    await ask(page, first, answers[first])
    const before = await savedChat(first, 2)
    await ask(page, second, answers[second])
    await savedChat(first, 4)

    // The model was asked the follow-up with the first question and answer before it.
    const followUp = chatRequests(model).find(
      ({ body }) => lastUserText(body) === second,
    )
    expect(conversationSent(followUp?.body ?? {})).toEqual([
      first,
      answers[first],
      second,
    ])

    await page.reload()
    await expect(listed(page, first)).toHaveCount(1)
    await expect(listed(page, second)).toHaveCount(0)
    await listed(page, first).click()
    await expect(chatColumn(page).getByText(answers[second])).toBeVisible()
    expectInOrder((await readChat(page)).text, [
      first,
      answers[first],
      second,
      answers[second],
    ])

    // When the chat was created is not shown anywhere; the archive keeps it.
    const after = (await savedChats()).filter((chat) => chat.title === first)
    expect(after).toHaveLength(1)
    expect(after[0].id).toBe(before.id)
    expect(after[0].createdAt).toBe(before.createdAt)
    expect(after[0].messageCount).toBe(4)
    const saved = await backend<{ messages: SavedMessage[] } | null>(
      'load_ai_conversation',
      { conversationId: before.id },
    )
    expect(saved?.messages.map(({ role, content }) => [role, content])).toEqual(
      [
        ['user', first],
        ['assistant', answers[first]],
        ['user', second],
        ['assistant', answers[second]],
      ],
    )
  } finally {
    await model.close()
  }
})

test('a deleted chat is gone with the pages it cited, and deleting it again deletes nothing', async ({
  page,
}) => {
  const question = 'Where did I read about spawn_blocking running dry?'
  const later = 'What is a good name for a cat?'
  const answers = {
    [question]: 'You read it in the tokio issue listed below.',
    [later]: 'Miso is a good name for a cat.',
  }
  const searchFirst = searchThenAnswer({ query: 'spawn_blocking' }, answers)
  const model = await startService(page, 'E2E delete', (body) =>
    // The later question is answered without searching.
    lastUserText(body) === later
      ? { content: answers[later] }
      : searchFirst(body),
  )
  try {
    await ask(page, question, answers[question])
    const cited = chatColumn(page)
      .getByRole('group', { name: 'Pages used' })
      .getByRole('link', { name: ISSUE_TITLE })
    await expect(cited).toBeVisible()
    const chat = await savedChat(question, 2)
    const kept = await backend<{ messages: SavedMessage[] } | null>(
      'load_ai_conversation',
      { conversationId: chat.id },
    )
    expect(
      kept?.messages[1].citations?.length,
      'the archive holds the pages the answer used',
    ).toBeGreaterThan(0)

    const row = listed(page, question)
    await row.hover()
    await row
      .locator('xpath=..')
      .getByRole('button', { name: 'Conversation options' })
      .click()
    await page.getByRole('menuitem', { name: 'Delete' }).click()
    const confirm = page.getByRole('alertdialog', {
      name: 'Delete this conversation?',
    })
    await expect(confirm).toContainText(`“${question}” will be removed`)
    await confirm.getByRole('button', { name: 'Delete' }).click()

    // The open chat was the one deleted: Ask starts a new one.
    await expect(row).toHaveCount(0)
    await expect(chatColumn(page).getByText(answers[question])).toHaveCount(0)
    await expect(cited).toHaveCount(0)
    await page.reload()
    await expect(conversationList(page)).toBeVisible()
    await expect(listed(page, question)).toHaveCount(0)

    expect(
      await backend('load_ai_conversation', { conversationId: chat.id }),
    ).toBeNull()
    expect(
      await backend('delete_ai_conversation', { conversationId: chat.id }),
    ).toEqual({ deleted: false })
    expect((await savedChats()).map(({ id }) => id)).not.toContain(chat.id)

    // A new chat, opened again after a reload, shows none of those pages.
    await ask(page, later, answers[later])
    await savedChat(later, 2)
    await page.reload()
    await listed(page, later).click()
    await expect(chatColumn(page).getByText(answers[later])).toBeVisible()
    await expect(
      chatColumn(page).getByRole('group', { name: 'Pages used' }),
    ).toHaveCount(0)
    await expect(page.getByText(ISSUE_TITLE)).toHaveCount(0)
  } finally {
    await model.close()
  }
})

test('a chat opened again shows its question, thinking, search, answer and pages as before', async ({
  page,
}, testInfo) => {
  const question = 'Find the issue where spawn_blocking ran out of threads'
  const thinking =
    'They want a GitHub issue about spawn_blocking, so the archive search should find it.'
  const answer = 'It was the tokio issue on spawn_blocking pool exhaustion.'
  const model = await startService(page, 'E2E reopen', (body) => {
    if (body.messages?.at(-1)?.role === 'tool' || !offersTools(body))
      return { content: answer }
    return {
      reasoning: thinking,
      toolCalls: [
        { name: 'search_history', arguments: { query: 'spawn_blocking' } },
      ],
    }
  })
  try {
    await ask(page, question, answer)
    await savedChat(question, 2)
    await expect(
      chatColumn(page).getByRole('button', {
        name: 'Thinking · searched the archive 1 time',
      }),
    ).toBeVisible()
    const shown = await readChat(page)
    const search = 'search_history({"query":"spawn_blocking"})'
    expectInOrder(shown.text, [
      question,
      thinking,
      search,
      '→ search_history: ',
      answer,
    ])
    expect(
      shown.pages.some((title) => title?.startsWith(`${ISSUE_TITLE} · `)),
      'the issue is among the pages used',
    ).toBe(true)

    await page.reload()
    await expect(listed(page, question)).toHaveCount(1)
    await listed(page, question).click()
    await expect(chatColumn(page).getByText(answer)).toBeVisible()
    const reopened = await readChat(page)
    expect(reopened.text).toBe(shown.text)
    // The same pages, each with the day of its visit. Their order is not
    // compared: live they come in the search's order, reopened in visit-id
    // order, so "Open in History" (the first page) can change too.
    expect(reopened.pages).toEqual(shown.pages)
    // Recorded, not compared: live, the token count is the last model
    // request's; reopened, it is the whole answer's.
    await testInfo.attach('shown and reopened', {
      body: JSON.stringify(
        {
          tokens: [shown.tokens, reopened.tokens],
          pageOrder: [shown.shownPages, reopened.shownPages],
          openInHistory: [shown.openInHistory, reopened.openInHistory],
        },
        null,
        2,
      ),
      contentType: 'application/json',
    })

    const saved = (await savedChats()).filter((chat) => chat.title === question)
    expect(saved.map(({ title }) => title)).toEqual([question])
  } finally {
    await model.close()
  }
})

test('a model that keeps calling tools is stopped at the step limit and still answers', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  const stepLimit = rustConstant(
    'src-tauri/crates/vault-core/src/ai/agent_harness.rs',
    'DEFAULT_MAX_ITERATIONS',
  )
  const question = 'Tell me everything I ever read about tokio'
  const answer = 'Here is what the searches found before they were stopped.'
  // It calls the search whenever it may, and answers only when it may not.
  const model = await startService(page, 'E2E runaway', (body) =>
    offersTools(body)
      ? {
          toolCalls: [
            { name: 'search_history', arguments: { query: 'tokio' } },
          ],
        }
      : { content: answer },
  )
  try {
    const before = chatRequests(model).length
    const composer = page.getByRole('textbox', { name: 'Message' })
    await composer.fill(question)
    await composer.press('Enter')
    const column = chatColumn(page)
    await expect(column.getByText(answer)).toBeVisible({ timeout: 240_000 })
    await expect(
      column.getByText(
        'It stopped after reaching the search step limit, so the answer may be incomplete.',
      ),
    ).toBeVisible()
    await expect(
      column.getByRole('button', {
        name: `Searched the archive ${stepLimit} times`,
      }),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: 'Send', exact: true }),
    ).toBeVisible()
    await savedChat(question, 2)

    const sent = chatRequests(model).slice(before)
    await expectCount(
      testInfo,
      'model requests: the step limit, then one answer without tools',
      sent.length,
      stepLimit + 1,
    )
    expect(
      sent.slice(0, stepLimit).every(({ body }) => offersTools(body)),
      'every step offered the tools',
    ).toBe(true)
    const last = sent[sent.length - 1].body
    expect(offersTools(last), 'the last request offers no tools').toBe(false)
    await expectCount(
      testInfo,
      'search results the last request carries',
      toolResults(last).length,
      stepLimit,
    )
    const callIds = (last.messages ?? [])
      .filter(({ role }) => role === 'tool')
      .map((message) => message.tool_call_id)
    expect(new Set(callIds).size, 'each result answers its own call').toBe(
      stepLimit,
    )
  } finally {
    await model.close()
  }
})

// Fails today: the search index sorts "oldest" by each page's newest visit and shows that
// visit, so a page visited across years never shows its first visit. Fixed and turned back on
// in WORK-SEARCH-FIRST-VISIT (docs/plan/STATUS.md).
test.fixme('“when did I first” gets the earliest visit across years, not a recent one', async ({
  page,
}, testInfo) => {
  const term = 'smol'
  const matches = archivedVisits()
    .filter((visit) =>
      `${visit.title} ${visit.url}`.toLowerCase().includes(term),
    )
    .sort((a, b) => a.at - b.at)
  const years = new Set(
    matches.map((visit) => new Date(visit.at).getFullYear()),
  )
  expect(
    years.size,
    `the fixture has “${term}” in more than one year`,
  ).toBeGreaterThan(1)
  const earliest = matches[0]

  const question = `When did I first read about ${term}?`
  const answer = 'The first time is the earliest visit below.'
  const model = await startService(
    page,
    'E2E first time',
    searchThenAnswer({ query: term, sort: 'oldest' }, { [question]: answer }),
  )
  try {
    await ask(page, question, answer)
    const answered = chatRequests(model).find(
      ({ body }) => toolResults(body).length > 0,
    )
    const result = toolResults(answered?.body ?? {}).join('\n')
    await testInfo.attach('search result sent to the model', {
      body: result,
      contentType: 'text/plain',
    })
    const rows = [
      ...result.matchAll(
        /^\[(\d+)\] (\d{4}-\d{2}-\d{2} \d{2}:\d{2}) \| (\S+) — (.*) \((\S+), /gm,
      ),
    ].map(([, id, time, url, title, reason]) => ({
      id,
      time,
      url,
      title,
      reason,
    }))
    expect(rows.length, 'the search found visits').toBeGreaterThan(0)
    expect(rows[0]).toMatchObject({
      time: localMinute(earliest.at),
      url: earliest.url,
      title: earliest.title,
      reason: 'lexical-date-ordered',
    })
    const times = rows.map(({ time }) => time)
    expect(times, 'oldest first').toEqual([...times].sort())
    expect(
      rows.map(({ url }) => url).slice(0, 3),
      'the first visits in the fixture, in order',
    ).toEqual(matches.slice(0, Math.min(3, rows.length)).map(({ url }) => url))

    // The user sees the same search and its first page among the ones used.
    await chatColumn(page)
      .getByRole('button', { name: 'Searched the archive 1 time' })
      .click()
    await expect(
      chatColumn(page).getByText(
        `search_history({"query":"${term}","sort":"oldest"})`,
      ),
    ).toBeVisible()
    await expect(
      chatColumn(page)
        .getByRole('group', { name: 'Pages used' })
        .getByRole('link', { name: earliest.title, exact: true }),
    ).toBeVisible()
  } finally {
    await model.close()
  }
})
