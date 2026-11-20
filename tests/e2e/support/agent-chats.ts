/**
 * The Agent space's History (the conversation list) and its reader, for
 * the agent-chats specs: opening them, finding a conversation's row, the
 * reader's parts, and what the backend says about the same conversations
 * (`list_agent_conversations`, `get_agent_conversation`, the window and
 * record reads), to check the screen against.
 *
 * Which saved conversation is a fixture conversation comes from the
 * fixture's facts (`fixture().agents`): its tool and its title, which is
 * the name the tool recorded or the first thing the person typed.
 */
import { readFileSync, rmSync, statSync } from 'node:fs'
import path from 'node:path'
import { expect, type Locator, type Page } from '@playwright/test'
import type {
  AgentConversation,
  AgentSubagent,
} from '../../../scripts/fixtures/synthetic-agents.mjs'
import { unlockIfLocked } from './app'
import { backend, fixture } from './fixture'

export interface WireChat {
  id: string
  harness: AgentConversation['tool']
  title: string | null
  titleSource: string
  project: { id: string; label: string } | null
  lastActivityAt: number | null
  starred: boolean
  archived: boolean
  trashed: boolean
  onlyInPathKeep: boolean
  tokensTotal: number | null
  childCount: number
  day?: string | null
}

export interface WirePage {
  items: WireChat[]
  groups: {
    day?: string | null
    count: number
    project?: { id: string; label: string } | null
    remaining?: number
  }[]
  nextCursor: string | null
}

export interface WireHeader {
  summary: WireChat
  publication: string
  nativeSessionId: string | null
  /** A checked resume command (`cd … && …`), or null: Continue copies the ID. */
  resumeCommand: string | null
  visibleRecords: number
  physicalRecords: number
  promptCount: number
  unparsedRecords: number
}

/**
 * The time zone History puts days in: the one the Insights on screen use
 * (`get_insights_time_zone`), which the page sends with every list read.
 */
async function shownZone() {
  return (await backend<{ shown: string }>('get_insights_time_zone', {})).shown
}

/** One page of the list, as the backend answers `request`. */
export async function wirePage(request: Record<string, unknown> = {}) {
  return backend<WirePage>('list_agent_conversations', {
    request: { timeZone: await shownZone(), ...request },
  })
}

/** Every row of the list for `request`, ungrouped, all pages. */
export async function wireList(request: Record<string, unknown> = {}) {
  const items: WireChat[] = []
  let cursor: string | null = null
  do {
    const page: WirePage = await wirePage({
      group: 'none',
      limit: 200,
      ...request,
      ...(cursor ? { cursor } : {}),
    })
    items.push(...page.items)
    cursor = page.nextCursor
  } while (cursor)
  return items
}

/** Every project group, all pages. */
export async function wireProjects() {
  const groups: WirePage['groups'] = []
  let cursor: string | null = null
  do {
    const page: WirePage = await wirePage({
      group: 'project',
      limit: 200,
      ...(cursor ? { cursor } : {}),
    })
    groups.push(...page.groups)
    cursor = page.nextCursor
  } while (cursor)
  return groups
}

export async function wireCounts(request: Record<string, unknown> = {}) {
  return backend<{
    matchingRoots: number
    spaceRoots: number
    spaceOnlyInPathKeep: number
    folderCount: number
  }>('get_agent_conversation_counts', {
    request: { timeZone: await shownZone(), ...request },
  })
}

export function wireHeader(id: string) {
  return backend<WireHeader>('get_agent_conversation', { request: { id } })
}

export function wireWindow<Row>(request: Record<string, unknown>) {
  return backend<{ total: number; rows: Row[] }>(
    'read_agent_conversation_window',
    { request },
  )
}

const plain = (text: string) => text.replace(/\s+/g, ' ').trim()

/** The fixture's conversation with this key, as the mutators left it. */
/**
 * A subagent's title (`agent-history.md` §4: a name you gave it, then the
 * tool's title, then what the parent wrote when it sent it, then its first
 * prompt): the description in the parent's `Task` call, read from the
 * parent's own file; the fixture gives its subagents no other title.
 */
export function subagentTitle(
  parent: AgentConversation,
  sub: AgentSubagent,
): string {
  for (const file of parent.files)
    for (const line of readFileSync(file, 'utf8').split('\n')) {
      if (!line.includes(sub.toolUseId)) continue
      const record = JSON.parse(line) as {
        message?: {
          content?: {
            type: string
            id?: string
            input?: { description?: unknown }
          }[]
        }
      }
      for (const part of record.message?.content ?? [])
        if (
          part.type === 'tool_use' &&
          part.id === sub.toolUseId &&
          typeof part.input?.description === 'string'
        )
          return part.input.description
    }
  return sub.firstUserMessage
}

export function agentChat(key: string): AgentConversation {
  const { agents } = fixture()
  const found = [...agents.conversations, ...agents.removed].find(
    (conversation) => conversation.key === key,
  )
  if (!found) throw new Error(`No conversation ${key} in the fixture`)
  return found
}

/**
 * The saved conversation that is the fixture's `conversation`: the one
 * with its title, or else the one whose session ID is the tool's.
 */
export async function savedChat(conversation: AgentConversation) {
  const rows = await wireList({ harnesses: [conversation.tool] })
  const title = plain(conversation.title)
  const titled = rows.filter(
    (row) => row.title !== null && plain(row.title) === title,
  )
  if (titled.length === 1) return titled[0]
  for (const row of rows)
    if ((await wireHeader(row.id)).nativeSessionId === conversation.nativeId)
      return row
  throw new Error(
    `No saved ${conversation.tool} conversation is “${title}” (${conversation.nativeId}); ${titled.length} have that title`,
  )
}

/**
 * Takes every mark off a conversation through the backend (no star, not
 * put away, out of the trash, the tool's title), so a spec that failed
 * half way leaves the archive as the backups did.
 */
export async function clearMarks(id: string) {
  const edit = (request: Record<string, unknown>) =>
    backend('edit_agent_conversation', { request: { id, ...request } })
  const { summary } = await wireHeader(id)
  if (summary.trashed) await edit({ action: 'restore' })
  if (summary.starred) await edit({ action: 'star', on: false })
  if (summary.archived) await edit({ action: 'archive', on: false })
  if (summary.titleSource === 'user')
    await edit({ action: 'rename', title: '' })
}

export async function openHistory(page: Page, search = '') {
  await page.goto(`/#/agent/chats${search}`)
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'History', level: 1 }),
  ).toBeVisible()
}

/** History with this conversation open beside the list. */
export async function openChat(page: Page, id: string) {
  await openHistory(page, `?chat=${encodeURIComponent(id)}`)
  await expect(readerTitle(page)).toBeVisible()
}

/** A search parameter of the hash route (`#/agent/chats?chat=…`). */
export function hashParam(page: Page, key: string) {
  const hash = new URL(page.url()).hash
  const at = hash.indexOf('?')
  return new URLSearchParams(at < 0 ? '' : hash.slice(at + 1)).get(key)
}

export const listRegion = (page: Page) =>
  page.getByRole('region', { name: 'Conversations', exact: true })

export const chatList = (page: Page) =>
  page.getByRole('group', {
    name: 'Conversation list; use the up and down arrow keys to switch',
  })

export const listRows = (page: Page) => chatList(page).locator('[data-chat-id]')

/** A conversation's row in the list, by its id. */
export const chatRow = (page: Page, id: string) =>
  chatList(page).locator(`[data-chat-id="${id}"]`)

/** Scrolls the list down until its last row is `id`, every page read. */
export async function expectLastRow(page: Page, id: string) {
  await expect(async () => {
    await chatList(page).evaluate((element) => {
      element.scrollTop = element.scrollHeight
    })
    await expect(listRows(page).last()).toHaveAttribute('data-chat-id', id, {
      timeout: 1_000,
    })
  }).toPass({ timeout: 30_000 })
}

/** Scrolls the list down until `target` is drawn (the list draws what is in view). */
export async function scrollListTo(page: Page, target: Locator) {
  await expect(async () => {
    if ((await target.count()) === 0)
      await chatList(page).evaluate((element) =>
        element.scrollBy(0, element.clientHeight / 2),
      )
    await expect(target).toBeVisible({ timeout: 1_000 })
  }).toPass({ timeout: 30_000 })
}

export const reader = (page: Page) =>
  page.getByRole('region', { name: 'Conversation', exact: true })

export const readerTitle = (page: Page) =>
  reader(page).locator('header').getByRole('heading').first()

/** The conversation's rows in the reader (not the raw records). */
export const transcript = (page: Page) =>
  reader(page).locator('[data-transcript]')

/** Picks an item of the reader's ⋯ menu. */
export async function readerMenu(page: Page, item: string | RegExp) {
  await reader(page).getByRole('button', { name: 'More', exact: true }).click()
  await page
    .getByRole('menu')
    .getByRole('menuitem', { name: item })
    .or(page.getByRole('menu').getByRole('menuitemcheckbox', { name: item }))
    .click()
}

export const filterButton = (page: Page) =>
  page.getByRole('button', {
    name: /^(Filter and sort|Filter \(\d+ conditions?\))$/,
  })

/** Opens the filter menu, presses a pill in one of its sections, closes it. */
export async function pickFilter(page: Page, section: string, pill: string) {
  await filterButton(page).click()
  await page
    .getByRole('dialog')
    .getByRole('group', { name: section, exact: true })
    .getByRole('button', { name: pill, exact: true })
    .click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toHaveCount(0)
}

/** The chip of a condition under the toolbar. */
export const chip = (page: Page, label: string) =>
  listRegion(page)
    .locator('span')
    .filter({ has: page.getByRole('button', { name: 'Remove this filter' }) })
    .filter({ hasText: label })

export const toast = (page: Page, text: string) =>
  page.locator('[data-sonner-toast]').filter({ hasText: text })

export function conversationsText(n: number) {
  return `${n.toLocaleString('en-US')} conversation${n === 1 ? '' : 's'}`
}

/** The switcher's line for Agent conversations says these two numbers. */
export async function expectSwitcherLine(
  page: Page,
  roots: number,
  onlyInPathKeep: number,
) {
  await page.getByRole('button', { name: /^Switch data type\. Now: / }).click()
  await expect(
    page
      .getByRole('menu', { name: /^Switch data type/ })
      .getByRole('menuitemradio', { name: /Agent conversations/ }),
  ).toContainText(
    `${conversationsText(roots)} · ${onlyInPathKeep} only in PathKeep`,
  )
  await page.keyboard.press('Escape')
}

/** Where exports land: the save dialog's suggested name in PATHKEEP_TEST_SAVE_DIR. */
export const savedDir = () => path.join(fixture().root, 'saved')

/**
 * The open reader's ⋯ › Export › `format` (a menu item's name starts with
 * it), and the file the backend says it wrote: `conversation.<extension>`
 * in the save folder, deleted first so an older one can't pass for it.
 */
export async function exportConversation(
  page: Page,
  format: RegExp,
  extension: string,
) {
  const file = path.join(savedDir(), `conversation.${extension}`)
  rmSync(file, { force: true })
  const answered = page.waitForResponse((response) =>
    response.url().endsWith('/commands/export_agent_conversation'),
  )
  await reader(page).getByRole('button', { name: 'More', exact: true }).click()
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click()
  await page.getByRole('menuitem', { name: format }).click()
  const response = await answered
  expect(response.ok()).toBe(true)
  const saved = (await response.json()) as { path: string; bytes: string }
  expect(saved.path).toBe(file)
  expect(statSync(saved.path).size).toBe(Number(saved.bytes))
  return saved
}
