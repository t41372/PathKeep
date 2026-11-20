/**
 * Find in this conversation (`agent-history.md` §4.4, §4.6), in the
 * reader beside History, on the fixture's scenes.
 *
 * Proves, against what the backend's own find counts
 * (`find_in_agent_conversation`, read to its last frame):
 * - ⌘F opens the find bar; a phrase the long conversation says once per
 *   run is found 112 times; the first is lit and current; Shift+Enter
 *   goes back from the first to the last, 112 turns and more than one
 *   window away, and Enter wraps forward to the first again;
 * - an occurrence inside a folded tool call (its command) opens the fold
 *   and the call, and is lit there;
 * - Esc closes the find bar first and the reader only after;
 * - a word only one subagent wrote opens that subagent in the side sheet,
 *   with the find bar in the sheet; Esc there closes find, then the sheet,
 *   and the reader stays open.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  agentChat,
  hashParam,
  openChat,
  readerTitle,
  savedChat,
  subagentTitle,
  transcript,
} from './support/agent-chats'
import { findHighlights, wireFind } from './support/agent-search'
import { fixture } from './support/fixture'

const findBar = (page: Page) =>
  page.getByRole('search', { name: 'Find in this conversation' })
const findInput = (page: Page) =>
  findBar(page).getByRole('textbox', { name: 'Find in this conversation' })

async function expectCurrent(page: Page, text: string, part: RegExp) {
  await expect
    .poll(async () => {
      const lit = await findHighlights(page)
      return `${lit.current?.toLowerCase()} | ${part.test(lit.part ?? '')}`
    })
    .toBe(`${text.toLowerCase()} | true`)
}

test('find steps through 112 occurrences across windows, both ways', async ({
  page,
}, testInfo) => {
  const large = fixture().agents.scenes.large
  const chat = await savedChat(agentChat(large.conversation))
  const phrase = '"within noise"'
  expect(await wireFind(chat.id, phrase)).toBe(large.turns)

  await openChat(page, chat.id)
  await page.keyboard.press('ControlOrMeta+f')
  await expect(findInput(page)).toBeFocused()
  await findInput(page).fill(phrase)
  await expect(findBar(page)).toContainText(`1 / ${large.turns}`, {
    timeout: 30_000,
  })
  await expect(findBar(page)).not.toContainText('…')
  await expectCurrent(page, 'within noise', /^Run 1:/)
  const lit = await findHighlights(page)
  expect(lit.lit.length, 'every occurrence on screen is lit').toBeGreaterThan(0)
  await testInfo.attach('the first of 112', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Back from the first is the last, far below.
  await findInput(page).press('Shift+Enter')
  await expect(findBar(page)).toContainText(`${large.turns} / ${large.turns}`)
  await expectCurrent(page, 'within noise', new RegExp(`^Run ${large.turns}:`))
  await expect(
    transcript(page).getByText(`Running run ${large.turns} now.`, {
      exact: true,
    }),
  ).toBeVisible()
  await testInfo.attach('the last of 112', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Forward from the last wraps to the first.
  await findInput(page).press('Enter')
  await expect(findBar(page)).toContainText(`1 / ${large.turns}`)
  await expectCurrent(page, 'within noise', /^Run 1:/)

  // In a folded tool call: the fold and the call open.
  await findInput(page).fill(`/save-baseline run-${large.turns}\\b/`)
  await expect(findBar(page)).toContainText('1 / 1', { timeout: 30_000 })
  await expectCurrent(page, `save-baseline run-${large.turns}`, /cargo bench/)
  await expect(
    transcript(page).getByRole('button', {
      name: new RegExp(`save-baseline run-${large.turns}`),
      expanded: true,
    }),
  ).toBeVisible()

  // Esc: find first, then the reader.
  await findInput(page).press('Escape')
  await expect(findBar(page)).toHaveCount(0)
  await expect(readerTitle(page)).toHaveText(chat.title!)
  expect((await findHighlights(page)).lit).toEqual([])
  await page.keyboard.press('Escape')
  await expect.poll(() => hashParam(page, 'chat')).toBeNull()
})

test('find goes into a subagent and keeps working in its sheet', async ({
  page,
}, testInfo) => {
  const parent = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(parent)
  const sub = parent.subagents[1]
  const word = sub.words[0]
  const occurrences = await wireFind(chat.id, word)
  expect(occurrences, 'only that subagent wrote it').toBeGreaterThan(0)

  await openChat(page, chat.id)
  await page.keyboard.press('ControlOrMeta+f')
  await findInput(page).fill(word)
  const sheet = page.getByRole('dialog')
  await expect(
    sheet.getByRole('heading', { name: subagentTitle(parent, sub) }),
  ).toBeVisible({ timeout: 30_000 })
  await expect(sheet.getByRole('search')).toContainText(`1 / ${occurrences}`)
  await expectCurrent(page, word, new RegExp(word, 'i'))
  await testInfo.attach('found in a subagent', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })

  // Esc closes find and leaves the sheet; Esc again closes the sheet.
  await page.keyboard.press('Escape')
  await expect(sheet.getByRole('search')).toHaveCount(0)
  await expect(sheet).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(sheet).toHaveCount(0)
  await expect(readerTitle(page)).toHaveText(chat.title!)
  expect(hashParam(page, 'chat')).toBe(chat.id)
})
