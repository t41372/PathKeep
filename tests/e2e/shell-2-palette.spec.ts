/**
 * ⌘K's Agent conversations and projects (`data-spaces.md` §6), on the
 * conversations the agent projects backed up (Claude Code, Codex, Pi).
 *
 * Proves:
 * - as soon as ⌘K opens, before anything is typed, it lists the recent
 *   conversations (the backend's, in its order, none put away) and the
 *   projects, with no row that searches for words not typed yet;
 * - a conversation put away leaves that list; typing words of its title
 *   finds it again, marked "Put away", above the row that searches the
 *   conversations; choosing it opens it on the History page with its title
 *   in the reader;
 * - words that match nothing leave only that search row, and no projects;
 * - a project opens its own page; the search row opens History with the
 *   words in its search box; "Background tasks" is among the pages to go to.
 *
 * Takes the "Put away" mark off again, as it found the conversation.
 */
import { randomUUID } from 'node:crypto'
import { expect, test, type Page } from '@playwright/test'
import { readerTitle } from './support/agent-chats'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

interface PaletteAnswer {
  conversations: {
    id: string
    title: string | null
    archived: boolean
    trashed: boolean
  }[]
  projects: { id: string; name: string; mainFolder: string | null }[]
}

// The backend takes a request's id only as a UUID, as the window sends it.
const answer = (text: string) =>
  backend<PaletteAnswer>('query_agent_palette', {
    request: { text, requestId: randomUUID() },
  })

const putAway = (id: string, on: boolean) =>
  backend('edit_agent_conversation', {
    request: { action: 'archive', id, on },
  })

async function openPalette(page: Page, words = '') {
  await page.keyboard.press('ControlOrMeta+k')
  const palette = page.getByRole('dialog', { name: 'Search and commands' })
  await expect(palette).toBeVisible()
  if (words) await palette.getByRole('combobox').fill(words)
  return palette
}

const chatsGroup = (page: Page) =>
  page
    .getByRole('dialog', { name: 'Search and commands' })
    .getByRole('group', { name: 'Agent conversations' })
const projectsGroup = (page: Page) =>
  page
    .getByRole('dialog', { name: 'Search and commands' })
    .getByRole('group', { name: 'Projects' })
const option = (page: Page, value: string) =>
  page
    .getByRole('dialog', { name: 'Search and commands' })
    .locator(`[cmdk-item][data-value="${value}"]`)
const hash = (page: Page) => () => new URL(page.url()).hash

/** The address's hash starts with `prefix` (the page may add to it). */
const startsWith = (prefix: string) =>
  new RegExp(`^${prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`)

test('⌘K lists recent conversations and projects, finds a put-away one by its title, and opens each', async ({
  page,
}, testInfo) => {
  await page.goto('/#/browsing/history')
  await unlockIfLocked(page)

  const recent = await answer('')
  expect(recent.conversations.length).toBeGreaterThan(1)
  expect(recent.conversations.length).toBeLessThanOrEqual(6)
  expect(
    recent.conversations.some((chat) => chat.archived || chat.trashed),
  ).toBe(false)
  expect(recent.projects.length).toBeGreaterThan(0)
  expect(recent.projects.length).toBeLessThanOrEqual(5)
  const target = recent.conversations.find(
    (chat) => (chat.title?.trim().length ?? 0) >= 8,
  )
  expect(target, 'a recent conversation with a title').toBeTruthy()
  const title = target!.title!.trim()

  // Nothing typed: the recent conversations and projects, in the backend's order.
  let palette = await openPalette(page)
  const chats = chatsGroup(page).getByRole('option')
  await expect(chats).toHaveCount(recent.conversations.length)
  for (const [index, chat] of recent.conversations.entries())
    await expect(chats.nth(index)).toHaveAttribute(
      'data-value',
      `agent-chat:${chat.id}`,
    )
  await expect(option(page, `agent-chat:${target!.id}`)).toContainText(title)
  const projects = projectsGroup(page).getByRole('option')
  await expect(projects).toHaveCount(recent.projects.length)
  for (const [index, project] of recent.projects.entries())
    await expect(projects.nth(index)).toContainText(project.name)
  await expect(
    palette.getByRole('option', { name: /^Search “Agent conversations”/ }),
  ).toHaveCount(0)
  await testInfo.attach('⌘K before anything is typed', {
    body: await palette.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')
  await expect(palette).toBeHidden()

  await putAway(target!.id, true)
  try {
    // Put away: not among the recent ones any more.
    const after = await answer('')
    expect(after.conversations.map((chat) => chat.id)).not.toContain(target!.id)
    palette = await openPalette(page)
    await expect(chats).toHaveCount(after.conversations.length)
    await expect(option(page, `agent-chat:${target!.id}`)).toHaveCount(0)

    // Words of its title find it, marked, above the search row: the
    // backend's answer for those words, in its order.
    const words = title.slice(0, 40).trim()
    const typed = await answer(words)
    const match = typed.conversations.find((chat) => chat.id === target!.id)
    expect(match?.archived, 'found by its title, put away').toBe(true)
    await palette.getByRole('combobox').fill(words)
    await expect(chats).toHaveCount(typed.conversations.length + 1)
    for (const [index, chat] of typed.conversations.entries())
      await expect(chats.nth(index)).toHaveAttribute(
        'data-value',
        `agent-chat:${chat.id}`,
      )
    const found = option(page, `agent-chat:${target!.id}`)
    await expect(found).toContainText('Put away')
    const searchRow = chatsGroup(page).getByRole('option', {
      name: `Search “Agent conversations” for “${words}”`,
    })
    await expect(searchRow).toBeVisible()
    await expect(chats.last()).toHaveAttribute('data-value', 'search:agent')
    await testInfo.attach('a put-away conversation found by its title', {
      body: await palette.screenshot(),
      contentType: 'image/png',
    })
    await found.click()
    await expect(palette).toBeHidden()
    await expect
      .poll(hash(page))
      .toMatch(
        startsWith(`#/agent/chats?chat=${encodeURIComponent(target!.id)}`),
      )
    await expect(readerTitle(page)).toHaveText(title)
  } finally {
    await putAway(target!.id, false)
  }

  // Words that match nothing: only the row that searches for them.
  await openPalette(page, 'zqxv no such words')
  await expect(chats).toHaveCount(1)
  await expect(chats.first()).toHaveAttribute('data-value', 'search:agent')
  await expect(projectsGroup(page)).toHaveCount(0)
  await page.keyboard.press('Escape')

  // A project opens its own page.
  const project = recent.projects[0]
  await openPalette(page)
  await option(page, `agent-project:${project.id}`).click()
  await expect
    .poll(hash(page))
    .toMatch(startsWith(`#/agent/projects/${encodeURIComponent(project.id)}`))
  await expect(
    page.getByRole('heading', { name: project.name, level: 1 }),
  ).toBeVisible()

  // The search row opens History with the words.
  palette = await openPalette(page, 'release')
  await palette
    .getByRole('option', {
      name: 'Search “Agent conversations” for “release”',
    })
    .click()
  await expect.poll(hash(page)).toMatch(startsWith('#/agent/chats?q=release'))

  // Background tasks is a page to go to.
  palette = await openPalette(page, 'background')
  await palette
    .getByRole('group', { name: 'Go to' })
    .getByRole('option', { name: 'Background tasks' })
    .click()
  await expect(page).toHaveURL(/#\/tasks$/)
  await expect(
    page.getByRole('heading', { name: 'Background tasks', level: 1 }),
  ).toBeVisible()
})
