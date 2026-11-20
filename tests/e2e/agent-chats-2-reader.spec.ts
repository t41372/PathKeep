/**
 * The conversation reader (`agent-history.md` §4) on the fixture's scenes,
 * beside the list and as a full page.
 *
 * Proves:
 * - a long conversation (112 turns, about 2 MB) opens at its start; "Jump
 *   to the end" shows the last turn; scrolling up reads the turns before
 *   the last window back in; the outline lists every prompt, highlights
 *   the one on screen and goes to a prompt in the middle;
 * - the raw records list every record PathKeep keeps; a record the reader
 *   does not show opens to exactly the JSON the tool wrote; "See in
 *   conversation" goes back to the conversation; the header says how many
 *   records could not be read, as the backend counts them;
 * - Pi's rewritten prompt shows the version the tool used last, and the
 *   other version with the branch that followed it;
 * - the three subagents sent together fold into one line; one opens in a
 *   side sheet with its own transcript; Esc closes the sheet, not the
 *   reader;
 * - the info panel shows the tokens and the cost Pi recorded, and where
 *   it happened; Continue copies the session ID the tool gave;
 * - a conversation only PathKeep still has says so, cannot be continued,
 *   and has no file to show in the file manager;
 * - a pasted screenshot is drawn from the saved bytes at its own size;
 * - a conversation whose working folder is gone says so in Continue and
 *   in the info panel;
 * - "Open full page" opens it at its own address with the outline; its
 *   project opens History filtered to it; Esc goes back; an address naming
 *   no conversation says so and leads to History.
 */
import { expect, test } from '@playwright/test'
import { TOOL_NAMES } from './support/agent-tools'
import {
  agentChat,
  chip,
  hashParam,
  openChat,
  openHistory,
  reader,
  readerMenu,
  readerTitle,
  savedChat,
  transcript,
  wireHeader,
  wireWindow,
} from './support/agent-chats'
import { unlockIfLocked } from './support/app'
import { fixture } from './support/fixture'

test.use({ permissions: ['clipboard-read', 'clipboard-write'] })

const WINDOW = 100

test('a long conversation: the end, back up through the windows, the outline', async ({
  page,
}, testInfo) => {
  const large = fixture().agents.scenes.large
  const chat = await savedChat(agentChat(large.conversation))
  await openChat(page, chat.id)
  const header = await wireHeader(chat.id)
  expect(header.promptCount).toBe(large.turns)
  expect(header.visibleRecords, 'several windows of rows').toBeGreaterThan(
    3 * WINDOW,
  )
  const rows = transcript(page)
  await expect(readerTitle(page)).toHaveText(chat.title!)
  await expect(rows.getByText(`(${large.firstWord})`)).toBeVisible()

  // The end: the last turn, and nothing of the first is drawn any more.
  await readerMenu(page, 'Jump to the end')
  await expect(rows.getByText(`(${large.lastWord})`)).toBeVisible()
  await expect(
    rows.getByText(`Running run ${large.turns} now.`, { exact: true }),
  ).toBeVisible()
  await expect(rows.getByText(`(${large.firstWord})`)).toHaveCount(0)
  await testInfo.attach('the end of the long conversation', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Back up past the last window: a turn whose prompt is in the one before.
  const prompts = await wireWindow<{ turn: number; visibleRank: number }>({
    id: chat.id,
    publication: header.publication,
    view: 'prompts',
    anchor: { kind: 'end' },
    limit: 200,
  })
  const lastWindow = Math.floor((header.visibleRecords - 1) / WINDOW) * WINDOW
  const before = prompts.rows.filter((row) => row.visibleRank < lastWindow)
  const earlier = before.at(-1)!
  expect(earlier, 'a prompt before the last window').toBeDefined()
  const earlierReply = rows.getByText(`Running run ${earlier.turn} now.`, {
    exact: true,
  })
  await expect(earlierReply).toHaveCount(0)
  await expect(async () => {
    await rows.evaluate((element) => element.scrollBy(0, -400))
    await expect(earlierReply).toBeVisible({ timeout: 1_500 })
  }).toPass({ timeout: 60_000 })

  // The outline: every prompt, the one on screen highlighted, the middle one.
  await readerMenu(page, 'What you said (outline)')
  const outline = reader(page).getByRole('navigation', {
    name: 'What you said',
  })
  await expect(outline.getByRole('heading')).toHaveText(
    `What you said · ${large.turns}`,
  )
  await expect(outline.locator('button[aria-current="true"]')).toBeVisible()
  await outline.locator(':scope > div').evaluate((element) => {
    element.scrollTop = (element.scrollHeight - element.clientHeight) / 2
  })
  const middle = outline.getByRole('button', {
    name: new RegExp(`\\(${large.middleWord}\\)`),
  })
  await middle.click()
  await expect(rows.getByText(`(${large.middleWord})`)).toBeVisible()
  await expect(middle).toHaveAttribute('aria-current', 'true')
  await testInfo.attach('the middle prompt, from the outline', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('the raw records: every record as written, and back to the conversation', async ({
  page,
}) => {
  const scene = fixture().agents.scenes.unknownEvents
  const chat = await savedChat(agentChat(scene.conversation))
  await openChat(page, chat.id)
  const header = await wireHeader(chat.id)

  // The header says how many records the conversation could not use.
  const unparsed = reader(page).getByText(
    /couldn’t be read into the conversation/,
  )
  if (header.unparsedRecords > 0)
    await expect(unparsed).toContainText(
      `${header.unparsedRecords} record${header.unparsedRecords === 1 ? '' : 's'}`,
    )
  else await expect(unparsed).toHaveCount(0)

  await readerMenu(page, `Raw records (${header.physicalRecords})`)
  await expect(reader(page).getByText('Viewing the raw records.')).toBeVisible()
  const raw = reader(page).getByRole('region', { name: 'Raw records' })
  await expect(raw).toContainText(
    `${header.physicalRecords} raw records, kept by PathKeep exactly as written.`,
  )

  // A record no reader knows: not shown, and its JSON is what the tool wrote.
  const unknown = scene.records[0]
  const item = raw.locator('[data-index]').filter({
    has: page.getByRole('button', {
      name: `Show record #${unknown.line + 1}`,
      exact: true,
    }),
  })
  await expect(item).toContainText('Not shown')
  await item
    .getByRole('button', {
      name: `Show record #${unknown.line + 1}`,
      exact: true,
    })
    .click()
  const json = item.locator('pre')
  await expect(json).toBeVisible()
  expect(JSON.parse((await json.textContent()) ?? '')).toEqual(unknown.record)

  // A record the conversation shows leads back to it.
  const window = await wireWindow<{
    ordinal: number
    visible: boolean
    visibleRank: number | null
  }>({
    id: chat.id,
    publication: header.publication,
    view: 'raw',
    anchor: { kind: 'start' },
    limit: 20,
  })
  const shown = window.rows.find((row) => row.visible)!
  expect(shown, 'a record the conversation shows').toBeDefined()
  await raw
    .locator('[data-index]')
    .filter({
      has: page.getByRole('button', {
        name: `Show record #${shown.ordinal + 1}`,
        exact: true,
      }),
    })
    .getByRole('button', { name: 'See in conversation' })
    .click()
  await expect(raw).toHaveCount(0)
  await expect(reader(page).getByText('Viewing the raw records.')).toHaveCount(
    0,
  )
  await expect(transcript(page)).toBeVisible()
})

test('a rewritten prompt: the version the tool used last, and the one before', async ({
  page,
}, testInfo) => {
  const branch = fixture().agents.scenes.branch
  const [older, newer] = branch.words
  const chat = await savedChat(agentChat(branch.conversation))
  await openChat(page, chat.id)
  const rows = transcript(page)
  await expect(rows.getByText('Version 2 of 2')).toBeVisible()
  await expect(rows.getByText(newer).first()).toBeVisible()
  await expect(rows.getByText(older)).toHaveCount(0)

  await rows.getByRole('button', { name: 'Previous version' }).click()
  await expect(rows.getByText('Version 1 of 2')).toBeVisible()
  await expect(rows.getByText(older).first()).toBeVisible()
  await expect(rows.getByText(newer)).toHaveCount(0)
  await expect(
    rows.getByText(/^This is the version before you rewrote it;/),
  ).toBeVisible()
  await testInfo.attach('the version before the rewrite', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  await rows.getByRole('button', { name: 'Next version' }).click()
  await expect(rows.getByText('Version 2 of 2')).toBeVisible()
  await expect(rows.getByText(newer).first()).toBeVisible()
  await expect(rows.getByText(older)).toHaveCount(0)
  await expect(
    rows.getByText(/^This is the version before you rewrote it;/),
  ).toHaveCount(0)
})

test('subagents sent together, one opened in the side sheet', async ({
  page,
}, testInfo) => {
  const parent = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(parent)
  expect(chat.childCount).toBe(parent.subagents.length)
  await openChat(page, chat.id)
  const rows = transcript(page)
  await rows
    .getByRole('button', {
      name: new RegExp(`^Sent ${parent.subagents.length} subagents`),
    })
    .click()
  const sub = parent.subagents[1]
  await rows
    .getByRole('button', { name: sub.firstUserMessage, exact: false })
    .click()
  const sheet = page.getByRole('dialog')
  await expect(
    sheet.getByRole('heading', { name: sub.firstUserMessage }),
  ).toBeVisible()
  await expect(sheet).toContainText('subagent · general-purpose')
  await expect(
    sheet.locator('[data-transcript]').getByText(sub.words[0]).first(),
  ).toBeVisible()
  await testInfo.attach('a subagent in the side sheet', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')
  await expect(sheet).toHaveCount(0)
  await expect(readerTitle(page)).toHaveText(chat.title!)
  expect(hashParam(page, 'chat')).toBe(chat.id)
})

test('the info panel and Continue, on what Pi recorded', async ({
  page,
}, testInfo) => {
  const scene = fixture().agents.scenes.piUsage
  const conversation = agentChat(scene.conversation)
  const chat = await savedChat(conversation)
  await openChat(page, chat.id)

  await reader(page).getByRole('button', { name: 'Info', exact: true }).click()
  const panel = page.getByRole('complementary', {
    name: 'About this conversation',
  })
  const { input, output, cacheRead, cacheWrite, cost } = scene.usage
  await expect(panel).toContainText(
    (input + output + cacheRead + cacheWrite).toLocaleString('en-US'),
  )
  await expect(panel).toContainText(
    new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 4,
    }).format(cost!),
  )
  await expect(panel).toContainText('Pi recorded this amount at API prices')
  await expect(panel).toContainText(conversation.projectFolder)
  await panel.getByRole('button', { name: 'Saved', exact: true }).click()
  await expect(panel).toContainText('Still in the tool’s folder too')
  await testInfo.attach('the info panel', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Continue: Pi's resume command is not confirmed, so the session ID.
  await reader(page).getByRole('button', { name: 'Continue' }).click()
  const popover = page.getByRole('dialog')
  await expect(popover).toContainText(
    `PathKeep hasn’t confirmed how this version of ${TOOL_NAMES.pi} resumes a conversation, so it can only copy the session ID.`,
  )
  await popover.getByRole('button', { name: 'Copy session ID' }).click()
  await expect(
    page
      .locator('[data-sonner-toast]')
      .filter({ hasText: 'Session ID copied' }),
  ).toBeVisible()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    conversation.nativeId,
  )
})

test('a conversation only PathKeep still has', async ({ page }) => {
  const removed = fixture().agents.removed.find(
    (conversation) => conversation.tool === 'codex',
  )!
  expect(removed, 'the conversation deleted from Codex’s folder').toBeDefined()
  const chat = await savedChat(removed)
  expect(chat.onlyInPathKeep).toBe(true)
  await openChat(page, chat.id)
  await expect(reader(page)).toContainText(
    `${TOOL_NAMES.codex} has deleted this conversation; this is the version PathKeep saved`,
  )
  await reader(page).getByRole('button', { name: 'Continue' }).click()
  await expect(page.getByRole('dialog')).toContainText(
    `${TOOL_NAMES.codex}’s folder no longer has this conversation, so it can’t be continued in ${TOOL_NAMES.codex}.`,
  )
  await page.keyboard.press('Escape')
  await reader(page).getByRole('button', { name: 'More', exact: true }).click()
  await expect(
    page.getByRole('menuitem', {
      name: /^Show in (Finder|File Explorer|file manager)$/,
    }),
  ).toBeDisabled()
  await page.keyboard.press('Escape')
})

test('a pasted screenshot, drawn from the bytes PathKeep saved', async ({
  page,
}) => {
  const scene = fixture().agents.scenes.attachments
  const chat = await savedChat(agentChat(scene.conversation))
  await openChat(page, chat.id)
  const image = transcript(page).locator('img').first()
  await expect(image).toBeVisible()
  await expect
    .poll(() =>
      image.evaluate((element) => [
        (element as HTMLImageElement).naturalWidth,
        (element as HTMLImageElement).naturalHeight,
      ]),
    )
    .toEqual([scene.image.width, scene.image.height])
})

test('a conversation whose working folder was deleted', async ({ page }) => {
  const scene = fixture().agents.scenes.missingFolder
  const chat = await savedChat(agentChat(scene.conversation))
  await openChat(page, chat.id)
  await reader(page).getByRole('button', { name: 'Continue' }).click()
  await expect(page.getByRole('dialog')).toContainText(
    'The working folder has been deleted. To continue where it was, make the folder again at the same path first',
  )
  await page.keyboard.press('Escape')
  await reader(page).getByRole('button', { name: 'Info', exact: true }).click()
  const panel = page.getByRole('complementary', {
    name: 'About this conversation',
  })
  await expect(panel).toContainText(scene.folder)
  await expect(panel).toContainText('folder no longer on disk')
})

test('the full page, its project, and an address with no conversation', async ({
  page,
}, testInfo) => {
  const conversation = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(conversation)
  await openChat(page, chat.id)
  await readerMenu(page, 'Open full page')
  await expect(page).toHaveURL(
    new RegExp(
      `#/agent/chat/${encodeURIComponent(chat.id).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`,
    ),
  )
  await expect(
    page.getByRole('heading', { name: chat.title!, level: 1 }),
  ).toBeVisible()
  await expect(
    page.getByRole('navigation', { name: 'What you said' }),
  ).toBeVisible()
  await expect(page.getByRole('button', { name: 'Close (Esc)' })).toHaveCount(0)
  await testInfo.attach('the full page', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Its project: History, filtered to it.
  await reader(page)
    .locator('header')
    .getByRole('button', { name: chat.project!.label, exact: true })
    .click()
  await expect.poll(() => hashParam(page, 'project')).toBe(chat.project!.id)
  await expect(page).toHaveURL(/#\/agent\/chats\?/)
  await expect(chip(page, chat.project!.label)).toBeVisible()
  await page.goBack()
  await expect(
    page.getByRole('heading', { name: chat.title!, level: 1 }),
  ).toBeVisible()

  // Esc goes back to where it was opened from.
  await page.keyboard.press('Escape')
  await expect.poll(() => hashParam(page, 'chat')).toBe(chat.id)
  await expect(readerTitle(page)).toHaveText(chat.title!)

  // An address that names no conversation.
  await page.goto('/#/agent/chat/no-such-conversation')
  await unlockIfLocked(page)
  await expect(page.getByText('Couldn’t find this conversation')).toBeVisible()
  await page.getByRole('link', { name: 'Open History' }).click()
  await expect(page).toHaveURL(/#\/agent\/chats$/)
  await openHistory(page)
})
