/**
 * Ask quoting saved Agent conversations, behind "Assistant can read Agent
 * conversations" (Settings › Agent conversations), against a scripted
 * OpenAI-compatible service (`support/fake-model.ts`) and the conversations
 * the agent projects before this one backed up.
 *
 * The cited record is the long Claude Code session's middle prompt (turn 57
 * of 112), whose made-up word no other conversation has: a reader that
 * opened at the start would not show it.
 *
 * Proves:
 * - A new chat says what the assistant reads: with the switch on, the
 *   browsing history and the Agent conversations; with it off, the browsing
 *   history, and that the conversations are read only once allowed in
 *   Settings › Agent conversations. Either way, with this service on this
 *   computer, that nothing leaves it.
 * - With the switch on, the model is offered `search_agent_conversations`;
 *   asked for the word, PathKeep sends back that prompt and its
 *   `pathkeep://conversation/open?…` link, and nothing from any other
 *   conversation. The answer cites the link inline. Under it, the chip
 *   names the conversation by its own title (not the passage) with its
 *   tool, in a group called "Agent conversations used", and the answer's
 *   button says it opens the Agent conversation. The chip, the inline link
 *   and the button all lead to the record the tool returned. The saved
 *   transcript carries the record as its target, and reopened from the list
 *   the Ask chat shows the same chip.
 * - With the switch off, a new question is not offered the tool, and a
 *   follow-up in the chat that cited the record still sends both questions
 *   but not the answer that quoted it. Nothing sent after the switch went
 *   off holds the word, the passage, a conversation link or the tool's name.
 *   No answer cites a conversation; the earlier chip, being this computer's
 *   own record, stays.
 * - In the reopened chat, the inline link opens the record inside the app,
 *   and so does the chip: Agent › History with the conversation open beside
 *   it, that prompt in view and the first turn not drawn. (Last, so the
 *   access checks above do not depend on the reader.)
 *
 * Assumes (from the `agent-ai` project order): Agent conversations are on
 * and backed up, and their Records search index is built. App Lock may be
 * on. Puts back the AI settings and the switch it found, and deletes the two
 * Ask chats it made, even when a step fails.
 */
import { expect, test, type Browser, type Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import {
  agentChat,
  hashParam,
  reader,
  readerTitle,
  savedChat,
  transcript,
  wireHeader,
} from './support/agent-chats'
import { wireSearch } from './support/agent-search'
import { addService } from './support/ai-service'
import { unlockIfLocked } from './support/app'
import {
  chatRequests,
  lastUserText,
  offersTools,
  startFakeModel,
  textOf,
  type FakeModel,
  type FakeRequest,
} from './support/fake-model'
import { backend, fixture } from './support/fixture'

test.describe.configure({ mode: 'serial' })

const KEY = 'e2e-agent-ai-key'
const SERVICE = 'E2E agent conversations'
const TOOL = 'search_agent_conversations'

// The questions never name the word: whatever the provider receives with
// the switch off can then be checked whole for it.
const FIRST_QUESTION = 'Which benchmark prompt had the odd marker word?'
const NO_ACCESS_QUESTION = 'Did I ask about the p95 in a coding session?'
const FOLLOW_UP = 'Thanks. Was that the warm-cache run?'
const ANSWER_LEAD = 'You asked it in the long benchmark session:'
const LINK_TEXT = 'open that prompt'
const NO_ACCESS_ANSWER = 'I can’t read your Agent conversations right now.'
const FOLLOW_UP_ANSWER = 'I can only go by what you wrote here.'

interface Passage {
  citation: string
  passage: string
}

interface ViewerPrefs {
  assistantMayRead: boolean
}

interface SavedCitation {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  target?: {
    kind: string
    conversationId: string
    sourceId: string
    ordinal: number
  } | null
}

interface AppSnapshot {
  config: { ai: Record<string, unknown> } & Record<string, unknown>
}

const viewer = () =>
  backend<ViewerPrefs>('get_agent_viewer_preferences', { request: {} })
const setAssistantMayRead = (on: boolean) =>
  backend('set_agent_viewer_preferences', {
    request: { assistantMayRead: on },
  })
const snapshot = () => backend<AppSnapshot>('app_snapshot')

let saved: { viewer: ViewerPrefs; ai: AppSnapshot['config']['ai'] } | null =
  null
let model: FakeModel | null = null
/** The fixture conversation the word is in, as the backend lists it. */
let chat: { id: string; title: string }
let word: string
let firstWord: string
/** What the tool returned with the switch on, as the provider got it. */
let returned: Passage[] = []
/** Where the first of them opens the reader, as the chip links to it. */
let chipHref: string | null = null

/** The tools a request offered, by name. */
const toolNames = (body: FakeRequest['body']) =>
  (body.tools ?? []).map((tool) => tool.function?.name)

/** The passages in a `search_agent_conversations` result; none in an error. */
function passagesIn(result: string): Passage[] {
  try {
    const parsed: unknown = JSON.parse(result)
    return Array.isArray(parsed) ? (parsed as Passage[]) : []
  } catch {
    return []
  }
}

/** The record a `pathkeep://conversation/open?…` link names, read as a stranger would. */
function recordOfLink(link: string) {
  const url = new URL(link)
  expect(`${url.protocol}//${url.host}${url.pathname}`).toBe(
    'pathkeep://conversation/open',
  )
  return {
    conversationId: url.searchParams.get('conversationId'),
    sourceId: url.searchParams.get('sourceId'),
    ordinal: Number(url.searchParams.get('ordinal')),
  }
}

/** A hash route's `at` parameter (`#/agent/chats?chat=…&at=…`). */
function atOf(href: string | null) {
  const query = (href ?? '').indexOf('?')
  return new URLSearchParams(query < 0 ? '' : href!.slice(query + 1)).get('at')
}

const chatColumn = (page: Page) => page.getByRole('main').locator('aside + div')
const askList = (page: Page) =>
  page.getByRole('complementary', { name: 'Conversations' })
const recordChips = (page: Page) =>
  chatColumn(page)
    .getByRole('group', { name: 'Agent conversations used' })
    .getByRole('link')
/** What a new chat says the assistant reads, with the switch on and off. */
const STARTER_ON =
  'The assistant reads the browsing history and the Agent conversations in your archive. With a local model, nothing leaves this computer.'
const STARTER_OFF =
  'The assistant reads the browsing history in your archive. It doesn’t read your Agent conversations unless you allow it in Settings › Agent conversations. With a local model, nothing leaves this computer.'

const settingSwitch = (page: Page) =>
  page.locator('#settings-agent-assistantMayRead')

async function goto(page: Page, path: string) {
  await page.goto(path)
  await unlockIfLocked(page)
}

async function ask(page: Page, question: string, answer: string | RegExp) {
  const composer = page.getByRole('textbox', { name: 'Message' })
  await composer.fill(question)
  await composer.press('Enter')
  await expect(chatColumn(page).getByText(answer)).toBeVisible({
    timeout: 60_000,
  })
  await expect(
    page.getByRole('button', { name: 'Send', exact: true }),
  ).toBeVisible({ timeout: 60_000 })
}

/** Flips the switch in Settings › Agent conversations and waits for it to be saved. */
async function setSwitch(page: Page, on: boolean) {
  await goto(page, '/#/settings/agent')
  await expect(settingSwitch(page)).toHaveAttribute('aria-checked', String(!on))
  await settingSwitch(page).click()
  await expect.poll(async () => (await viewer()).assistantMayRead).toBe(on)
}

/** Every saved Ask chat with this title. */
async function askChats(title: string) {
  const found: { id: string; title: string }[] = []
  let cursor: string | null = null
  do {
    const listPage: {
      conversations: { id: string; title: string }[]
      nextCursor: string | null
    } = await backend('list_ai_conversations', {
      request: { limit: 100, cursor },
    })
    found.push(...listPage.conversations.filter((c) => c.title === title))
    cursor = listPage.nextCursor
  } while (cursor)
  return found
}

/** The reader beside Agent › History, open at the record `at`. */
async function expectReaderAt(page: Page, at: string) {
  await expect
    .poll(() => new URL(page.url()).hash)
    .toMatch(/^#\/agent\/chats\?/)
  await expect.poll(() => hashParam(page, 'chat')).toBe(chat.id)
  expect(hashParam(page, 'at')).toBe(at)
  await expect(readerTitle(page)).toHaveText(chat.title)
  await expect(transcript(page).getByText(`(${word})`)).toBeInViewport({
    timeout: 30_000,
  })
  // The first turn is 56 turns above: the reader did not read from the start.
  await expect(transcript(page).getByText(`(${firstWord})`)).toHaveCount(0)
}

/**
 * The app loaded and unlocked in a page of its own, for setup and teardown
 * through the backend: after a restart the archive stays locked until a
 * window unlocks it.
 */
async function unlocked(browser: Browser, then: () => Promise<void>) {
  const page = await browser.newPage({
    baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
  })
  try {
    await page.goto('/#/settings/agent')
    await unlockIfLocked(page)
    await then()
  } finally {
    await page.close()
  }
}

test.beforeAll(async ({ browser }) => {
  await unlocked(browser, setUp)
})

async function setUp() {
  const large = fixture().agents.scenes.large
  word = large.middleWord
  firstWord = large.firstWord
  const listed = await savedChat(agentChat(large.conversation))
  const header = await wireHeader(listed.id)
  expect(header.summary.title, 'the conversation has a title').toBeTruthy()
  chat = { id: listed.id, title: header.summary.title!.trim() }
  // The word is in this conversation alone, as Records search finds it.
  const { shown: timeZone } = await backend<{ shown: string }>(
    'get_insights_time_zone',
    {},
  )
  const found = await wireSearch({
    query: word,
    roles: ['user', 'agent', 'tool', 'thinking'],
    filters: { harnesses: [], flags: [], range: 'all', timeZone },
  })
  expect(found.order).toEqual([chat.id])

  saved = { viewer: await viewer(), ai: (await snapshot()).config.ai }
  // Off to start with, whatever the spec before left: the UI turns it on.
  await setAssistantMayRead(false)
  model = await startFakeModel({
    apiKey: KEY,
    script: (body) => {
      if (!offersTools(body)) return { content: 'OK' }
      const last = body.messages?.at(-1)
      if (last?.role === 'tool') {
        const [first] = passagesIn(textOf(last.content))
        if (!first) return { content: NO_ACCESS_ANSWER }
        return {
          content: `${ANSWER_LEAD} “${first.passage.trim()}” — [${LINK_TEXT}](${first.citation}).`,
        }
      }
      // With the earlier answer left out, rig may send both questions as one message.
      if (lastUserText(body).includes(FOLLOW_UP))
        return { content: FOLLOW_UP_ANSWER }
      if (toolNames(body).includes(TOOL))
        return { toolCalls: [{ name: TOOL, arguments: { query: word } }] }
      return { content: NO_ACCESS_ANSWER }
    },
  })
}

test.afterAll(async ({ browser }) => {
  try {
    await unlocked(browser, async () => {
      if (saved) {
        await setAssistantMayRead(saved.viewer.assistantMayRead)
        const { config } = await snapshot()
        await backend('save_config', { config: { ...config, ai: saved.ai } })
      }
      for (const title of [FIRST_QUESTION, NO_ACCESS_QUESTION])
        for (const { id } of await askChats(title))
          await backend('delete_ai_conversation', { conversationId: id })
    })
  } finally {
    await model?.close()
  }
})

test('with access on, Ask cites the record under the conversation’s title', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  await setSwitch(page, true)
  await addService(page, { name: SERVICE, address: model!.url, key: KEY })
  await goto(page, '/#/ask')
  // The new chat says what the assistant reads now, and that the service
  // (on this computer) keeps it here.
  await expect(chatColumn(page).getByText(STARTER_ON)).toBeVisible()
  await ask(page, FIRST_QUESTION, ANSWER_LEAD)

  // What the provider got: the tool offered, then that prompt and its link.
  const requests = chatRequests(model!)
    .map(({ body }) => body)
    .filter(offersTools)
  expect(toolNames(requests[0])).toContain(TOOL)
  const answered = requests.find(
    (body) => body.messages?.at(-1)?.role === 'tool',
  )!
  returned = passagesIn(textOf(answered.messages!.at(-1)!.content))
  await testInfo.attach('tool result', {
    body: JSON.stringify(returned, null, 2),
    contentType: 'application/json',
  })
  expect(returned.length, 'the prompt with the word').toBeGreaterThan(0)
  for (const item of returned)
    expect(recordOfLink(item.citation).conversationId).toBe(chat.id)
  const cited = returned[0]
  expect(cited.passage.toLowerCase()).toContain(word.toLowerCase())
  const record = recordOfLink(cited.citation)

  // The chip: the conversation's own title and tool, leading to the record
  // the tool's link names; the inline link and the button lead there too.
  const chip = recordChips(page).first()
  await expect(recordChips(page)).toHaveCount(returned.length)
  await expect(chip).toHaveText(chat.title)
  const passageStart = Array.from(cited.passage).slice(0, 80).join('')
  await expect(chip).toHaveAttribute(
    'title',
    `${chat.title} · Claude Code · “${passageStart.trim()}”`,
  )
  chipHref = await chip.getAttribute('href')
  expect(atOf(chipHref)).toBe(`${record.ordinal}:${record.sourceId}`)
  await expect(
    chatColumn(page).getByRole('link', { name: 'Open in History' }),
  ).toHaveCount(0)
  await expect(
    chatColumn(page).getByRole('link', { name: 'Open the Agent conversation' }),
  ).toHaveAttribute('href', chipHref!)
  await expect(
    chatColumn(page).getByRole('link', { name: LINK_TEXT }),
  ).toHaveAttribute('href', chipHref!)
  await testInfo.attach('the answer and its chip', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Saved with the record as its target (Ask saves after the answer, with
  // nothing on screen saying so).
  let historyId = 0
  await expect
    .poll(
      async () => {
        const [askChat] = await askChats(FIRST_QUESTION)
        if (!askChat) return null
        const { messages } = await backend<{
          messages: { role: string; citations?: SavedCitation[] | null }[]
        }>('load_ai_conversation', { conversationId: askChat.id })
        const first = messages.findLast(
          (message) => message.role === 'assistant',
        )?.citations?.[0]
        historyId = first?.historyId ?? 0
        return first ?? null
      },
      { timeout: 30_000 },
    )
    .toMatchObject({
      profileId: 'agent-history',
      url: cited.citation,
      title: passageStart,
      target: {
        kind: 'conversation',
        conversationId: chat.id,
        sourceId: record.sourceId,
        ordinal: record.ordinal,
      },
    })
  expect(historyId).toBeLessThan(0)

  // Reopened from the list: the same chip, to the same record.
  await goto(page, '/#/ask')
  await askList(page).getByRole('button', { name: FIRST_QUESTION }).click()
  await expect(chatColumn(page).getByText(ANSWER_LEAD)).toBeVisible()
  await expect(recordChips(page).first()).toHaveText(chat.title)
  await expect(recordChips(page).first()).toHaveAttribute('href', chipHref!)
})

test('with access off, nothing from the conversations reaches the service', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  expect(returned.length, 'the first test saw the passage').toBeGreaterThan(0)
  await setSwitch(page, false)
  const from = chatRequests(model!).length

  // A new question: the tool is not offered, and nothing is cited. The new
  // chat says the conversations aren't read, and where to allow it.
  await goto(page, '/#/ask')
  await expect(chatColumn(page).getByText(STARTER_OFF)).toBeVisible()
  await expect(chatColumn(page).getByText(STARTER_ON)).toHaveCount(0)
  await ask(page, NO_ACCESS_QUESTION, NO_ACCESS_ANSWER)
  await expect(chatColumn(page).locator('a[href^="#/agent/"]')).toHaveCount(0)
  await expect(
    chatColumn(page).getByRole('group', { name: /Agent conversations used$/ }),
  ).toHaveCount(0)

  // A follow-up where the record was cited: the chip stays on screen, but
  // the answer that quoted it is not sent again.
  await askList(page).getByRole('button', { name: FIRST_QUESTION }).click()
  await expect(recordChips(page).first()).toHaveText(chat.title)
  await ask(page, FOLLOW_UP, FOLLOW_UP_ANSWER)

  const sent = chatRequests(model!)
    .slice(from)
    .map(({ body }) => body)
  await testInfo.attach('requests with access off', {
    body: JSON.stringify(sent, null, 2),
    contentType: 'application/json',
  })
  expect(sent.length, 'the new question and the follow-up').toBe(2)
  for (const body of sent) {
    expect(toolNames(body)).not.toContain(TOOL)
    const wire = JSON.stringify(body).toLowerCase()
    for (const secret of [
      word,
      TOOL,
      'pathkeep://conversation',
      ...returned.map((item) => item.passage.trim()),
    ])
      expect(wire, secret).not.toContain(secret.toLowerCase())
  }
  const followUp = sent.at(-1)!
  const users = (followUp.messages ?? [])
    .filter((message) => message.role === 'user')
    .map((message) => textOf(message.content))
    .join('\n')
  expect(users).toContain(FIRST_QUESTION)
  expect(users).toContain(FOLLOW_UP)
  await testInfo.attach('the follow-up with access off', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('the inline link and the chip open the reader at the cited prompt', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  expect(chipHref, 'the first test saw the chip').toBeTruthy()
  const at = atOf(chipHref)!
  const reopen = async () => {
    await goto(page, '/#/ask')
    await askList(page).getByRole('button', { name: FIRST_QUESTION }).click()
    await expect(chatColumn(page).getByText(ANSWER_LEAD)).toBeVisible()
  }

  // The link the model wrote opens inside the app, not in the system's opener.
  await reopen()
  await chatColumn(page).getByRole('link', { name: LINK_TEXT }).click()
  await expectReaderAt(page, at)

  await reopen()
  await recordChips(page).first().click()
  await expectReaderAt(page, at)
  await expect(reader(page)).toBeVisible()
  await testInfo.attach('the reader at the cited prompt', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})
