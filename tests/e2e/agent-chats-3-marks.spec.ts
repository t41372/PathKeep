/**
 * The marks a person puts on a conversation (`agent-history.md` §8), from
 * the list's right-click menu and from the reader, and how the list, the
 * reader and the counts follow. Every mark is taken off again, so the
 * specs after this one see the archive as the backups left it.
 *
 * Proves, against the backend's header and lists:
 * - a star from the right-click menu shows on the row and puts it in
 *   "Starred"; taking it off in the reader takes it out at once;
 * - Rename changes the title in the reader and the list; an empty name
 *   brings back the tool's title;
 * - Put away hides the row but leaves the reader open and the switcher's
 *   count alone; Undo brings it back; under "Put away" it can be brought
 *   back from the right-click menu;
 * - Move to trash closes the reader, hides the row and takes it out of the
 *   switcher's count; the trash says it keeps conversations and never
 *   empties itself; the reader there offers Put back, which brings the row
 *   and the count back; Undo on the notice does the same.
 */
import { expect, test, type Page } from '@playwright/test'
import {
  chatList,
  chatRow,
  chip,
  clearMarks,
  expectSwitcherLine,
  hashParam,
  listRows,
  openHistory,
  pickFilter,
  reader,
  readerMenu,
  readerTitle,
  toast,
  wireCounts,
  wireHeader,
  wireList,
  type WireChat,
} from './support/agent-chats'

const rowMenu = async (page: Page, id: string, item: string) => {
  await chatRow(page, id).click({ button: 'right' })
  await page.getByRole('menu').getByRole('menuitem', { name: item }).click()
}

const removeChip = (page: Page, label: string) =>
  chip(page, label).getByRole('button', { name: 'Remove this filter' }).click()

/** The conversation the test marks; whatever happens, its marks come off. */
let marked: string | undefined
test.afterEach(async () => {
  if (marked) await clearMarks(marked)
})

test('star, rename, put away and the trash: the list, the reader and the counts follow', async ({
  page,
}, testInfo) => {
  await openHistory(page)
  const all = await wireList()
  // A recent conversation (a row in view) titled by its first prompt, still
  // in its tool's folder, that nothing has marked.
  const target: WireChat = all
    .slice(0, 8)
    .find(
      (row) =>
        row.titleSource === 'firstPrompt' &&
        !row.onlyInPathKeep &&
        !row.starred,
    )!
  expect(target, 'a plain conversation').toBeDefined()
  marked = target.id
  const title = target.title!
  const counts = await wireCounts()
  await expectSwitcherLine(page, counts.spaceRoots, counts.spaceOnlyInPathKeep)

  // Star from the right-click menu.
  await expect(chatRow(page, target.id)).toBeVisible()
  await rowMenu(page, target.id, 'Star')
  await expect(
    chatRow(page, target.id).getByRole('img', { name: 'Starred' }),
  ).toBeVisible()
  expect((await wireHeader(target.id)).summary.starred).toBe(true)
  await pickFilter(page, 'Only', 'Starred')
  const starred = await wireList({ flags: ['starred'] })
  expect(starred.map((row) => row.id)).toContain(target.id)
  await expect(listRows(page)).toHaveCount(starred.length)

  // Off again in the reader: out of "Starred" at once.
  await chatRow(page, target.id).click()
  await expect.poll(() => hashParam(page, 'chat')).toBe(target.id)
  const star = reader(page).getByRole('button', { name: 'Remove star' })
  await expect(star).toHaveAttribute('aria-pressed', 'true')
  await star.click()
  await expect(
    reader(page).getByRole('button', { name: 'Star', exact: true }),
  ).toHaveAttribute('aria-pressed', 'false')
  await expect(chatRow(page, target.id)).toHaveCount(0)
  expect((await wireHeader(target.id)).summary.starred).toBe(false)
  await removeChip(page, 'Starred')
  await expect(chatRow(page, target.id)).toBeVisible()

  // Rename, then an empty name: the tool's title again.
  await readerMenu(page, 'Rename…')
  const rename = page.getByRole('dialog', { name: 'Rename' })
  const name = rename.getByRole('textbox', { name: 'Name' })
  await expect(name).toHaveValue(title)
  await expect(rename.getByRole('button', { name: 'Save' })).toBeDisabled()
  await name.fill('Release audit, second pass')
  await rename.getByRole('button', { name: 'Save' }).click()
  await expect(toast(page, 'Name updated')).toBeVisible()
  await expect(rename).toHaveCount(0)
  await expect(readerTitle(page)).toHaveText('Release audit, second pass')
  await expect(chatRow(page, target.id)).toContainText(
    'Release audit, second pass',
  )
  expect((await wireHeader(target.id)).summary.titleSource).toBe('user')
  await testInfo.attach('renamed', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await readerMenu(page, 'Rename…')
  await name.fill('')
  await rename.getByRole('button', { name: 'Save' }).click()
  await expect(rename).toHaveCount(0)
  await expect(readerTitle(page)).toHaveText(title)
  await expect(chatRow(page, target.id)).toContainText(title)
  expect((await wireHeader(target.id)).summary.titleSource).toBe('firstPrompt')

  // Put away from the reader, and Undo on its notice (it shows for a few
  // seconds, so before anything else).
  await readerMenu(page, 'Put away')
  const putAway = toast(page, 'Put away. It no longer shows in the list')
  await expect(putAway).toBeVisible()
  await putAway.getByRole('button', { name: 'Undo' }).click()
  await expect(chatRow(page, target.id)).toBeVisible()
  expect((await wireHeader(target.id)).summary.archived).toBe(false)

  // Put away from the list: the row goes, the reader stays open and says
  // so, and the switcher still counts it.
  await rowMenu(page, target.id, 'Put away')
  await expect(chatRow(page, target.id)).toHaveCount(0)
  await expect(readerTitle(page)).toHaveText(title)
  await expect(
    reader(page).locator('header').getByRole('img', { name: 'Put away' }),
  ).toBeVisible()
  await expectSwitcherLine(page, counts.spaceRoots, counts.spaceOnlyInPathKeep)

  // Brought back from under "Put away".
  await pickFilter(page, 'Only', 'Put away')
  await expect(
    chatRow(page, target.id).getByRole('img', { name: 'Put away' }),
  ).toBeVisible()
  await rowMenu(page, target.id, 'Bring back')
  await expect(toast(page, 'Brought back')).toBeVisible()
  await expect(chatRow(page, target.id)).toHaveCount(0)
  await removeChip(page, 'Put away')
  await expect(chatRow(page, target.id)).toBeVisible()
  await expect(
    reader(page).locator('header').getByRole('img', { name: 'Put away' }),
  ).toHaveCount(0)
  expect((await wireHeader(target.id)).summary.archived).toBe(false)

  // The trash, from the right-click menu: the reader closes, the count drops.
  await rowMenu(page, target.id, 'Move to trash')
  await expect(toast(page, 'Moved to the trash').first()).toBeVisible()
  await expect.poll(() => hashParam(page, 'chat')).toBeNull()
  await expect(chatRow(page, target.id)).toHaveCount(0)
  await expectSwitcherLine(
    page,
    counts.spaceRoots - 1,
    counts.spaceOnlyInPathKeep,
  )
  await pickFilter(page, 'Only', 'Trash')
  await expect(
    page.getByText('The trash never empties itself.', { exact: false }),
  ).toBeVisible()
  await chatRow(page, target.id).click()
  await expect(
    reader(page).getByText('This conversation is in the trash.'),
  ).toBeVisible()
  await testInfo.attach('in the trash', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await reader(page)
    .locator('header')
    .getByRole('button', { name: 'Put back' })
    .click()
  await expect(toast(page, 'Put back')).toBeVisible()
  await expect(
    reader(page).getByText('This conversation is in the trash.'),
  ).toHaveCount(0)
  await expect(chatRow(page, target.id)).toHaveCount(0)
  await removeChip(page, 'Trash')
  await expect(chatRow(page, target.id)).toBeVisible()
  await expectSwitcherLine(page, counts.spaceRoots, counts.spaceOnlyInPathKeep)

  // The trash from the reader, which closes it, then Undo on its notice.
  await readerMenu(page, 'Move to trash')
  // The newest notice: the first one may still be on screen.
  const trashed = toast(page, 'Moved to the trash').last()
  await expect(trashed).toBeVisible()
  await trashed.getByRole('button', { name: 'Undo' }).click()
  expect(hashParam(page, 'chat')).toBeNull()
  await expect(chatRow(page, target.id)).toBeVisible()
  await expect(chatList(page)).toBeVisible()
  const after = await wireHeader(target.id)
  expect(after.summary).toMatchObject({
    starred: false,
    archived: false,
    trashed: false,
    titleSource: 'firstPrompt',
  })
  await expectSwitcherLine(page, counts.spaceRoots, counts.spaceOnlyInPathKeep)
})
