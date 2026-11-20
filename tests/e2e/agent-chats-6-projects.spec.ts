/**
 * Projects in the Agent space (`agent-history.md` §5) on the projects the
 * agent backups grouped, with History following every change.
 *
 * Proves, against what the backend lists (`list_agent_projects`,
 * `get_agent_project`, `list_agent_conversations`):
 * - the page counts every project and lists the ones not pinned under
 *   Other projects;
 * - Rename gives a project a name History shows on its rows and its
 *   filter; an empty name goes back to the folder's;
 * - Add folder takes another project's folders into this one: that
 *   project leaves the list, its address says it is gone, and History's
 *   project filter lists both projects' conversations; Undo on the notice
 *   brings it back, with History as it was;
 * - Pin, a new section named in place, and dragging: a pin dropped on the
 *   section goes into it, and a second pin dropped on the top half of the
 *   first goes before it; the order the backend returns is the order on
 *   screen;
 * - a project's page lists its conversations in the Records workbench,
 *   scoped (its own search box, no project filter); an address naming no
 *   project says so.
 */
import { expect, test } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import {
  chatList,
  expectLastRow,
  filterButton,
  hashParam,
  listRows,
  openHistory,
  toast,
  wireList,
} from './support/agent-chats'
import {
  dragTo,
  openProjects,
  otherCard,
  pinnedCard,
  projectMenu,
  projectNamed,
  projectRow,
  resetProjects,
  wireProject,
  wireProjectList,
} from './support/agent-projects'

test.afterEach(resetProjects)

test('rename and add a folder, then Undo, with History following', async ({
  page,
}, testInfo) => {
  await resetProjects()
  const tidepool = await projectNamed('tidepool')
  const list = await wireProjectList()
  await openProjects(page)
  await expect(
    page.getByText(new RegExp(`^${list.total} projects\\. A project`)),
  ).toBeVisible()
  const others = await wireProjectList({ otherOnly: true })
  await expect(
    otherCard(page).getByRole('heading', { name: 'Other projects' }),
  ).toBeVisible()
  await expect(otherCard(page).locator('h2 + span')).toHaveText(
    String(others.matching),
  )

  // Rename: History's rows and filter say the new name.
  await projectMenu(page, tidepool.id, 'tidepool', 'Rename…')
  const dialog = page.getByRole('dialog', { name: 'Rename' })
  await dialog.getByLabel('Name').fill('Tidepool lab')
  await dialog.getByRole('button', { name: 'Save' }).click()
  await expect(toast(page, 'Name updated')).toBeVisible()
  await expect(projectRow(page, tidepool.id)).toContainText('Tidepool lab')
  expect((await projectNamed('tidepool')).displayName).toBe('Tidepool lab')
  const inTidepool = await wireList({ projectId: tidepool.id })
  expect(inTidepool[0].project?.label).toBe('Tidepool lab')
  await openHistory(page, `?project=${encodeURIComponent(tidepool.id)}`)
  await expect(listRows(page).first()).toContainText('Tidepool lab')
  await filterButton(page).click()
  await expect(
    page
      .getByRole('dialog')
      .getByRole('button', { name: 'Project', exact: true }),
  ).toHaveText('Tidepool lab')
  await page.keyboard.press('Escape')

  // An empty name goes back to the folder's.
  await openProjects(page)
  await projectMenu(page, tidepool.id, 'Tidepool lab', 'Rename…')
  await dialog.getByLabel('Name').fill('')
  await dialog.getByRole('button', { name: 'Save' }).click()
  await expect(projectRow(page, tidepool.id)).toContainText('tidepool')
  expect((await projectNamed('tidepool')).displayName).toBe('tidepool')

  // Add folder: another project's conversations count here now.
  const candidates = await wireProjectList({ candidateFor: tidepool.id })
  const from =
    candidates.items.find((item) => item.defaultName === 'infra-scripts') ??
    candidates.items.find((item) => item.conversationCount > 0)
  expect(from, 'a project whose folders can be added').toBeDefined()
  const before = await wireList({ projectId: tidepool.id })
  const theirs = await wireList({ projectId: from!.id })
  await projectMenu(page, tidepool.id, 'tidepool', 'Project settings…')
  const settings = page.getByRole('dialog', { name: 'Project settings' })
  await settings.getByRole('button', { name: 'Add folder…' }).click()
  const add = page.getByRole('dialog', { name: 'Add a folder to “tidepool”' })
  await add
    .getByPlaceholder('Search folders or project names')
    .fill(from!.defaultName)
  await add.getByRole('option', { name: new RegExp(from!.defaultName) }).click()
  const added = toast(page, `to “tidepool”`)
  const count = from!.conversationCount
  await expect(added).toContainText(
    `${count} conversation${count === 1 ? '' : 's'} came along`,
  )
  // The notice waits while the pointer is on it (it has Undo).
  await added.hover()
  await expect(
    settings
      .locator('[data-folder-path]')
      .filter({ hasText: 'Added by you' })
      .first(),
  ).toBeVisible()
  await testInfo.attach('a folder added to tidepool', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  expect(await wireProject(from!.id)).toMatchObject({
    status: 'addedInto',
    into: tidepool.id,
  })
  const joined = await wireList({ projectId: tidepool.id })
  expect(joined).toHaveLength(before.length + theirs.length)
  expect(joined.map((chat) => chat.id)).toEqual(
    expect.arrayContaining(theirs.map((chat) => chat.id)),
  )

  // Undo puts it back, and the settings show tidepool's own folders.
  await added.getByRole('button', { name: 'Undo' }).click()
  await expect.poll(async () => (await wireProject(from!.id)).status).toBe('ok')
  expect(await wireList({ projectId: tidepool.id })).toHaveLength(before.length)
  await expect(
    settings.locator('[data-folder-path]').filter({ hasText: 'Added by you' }),
  ).toHaveCount(0)
  await settings.press('Escape')
  await expect(settings).toHaveCount(0)
  await expect(projectRow(otherCard(page), from!.id)).toBeVisible()

  // History, filtered to tidepool, lists only its own again.
  await openHistory(page, `?project=${encodeURIComponent(tidepool.id)}`)
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    before[0].id,
  )
  await expectLastRow(page, before.at(-1)!.id)
  await expect(chatList(page)).toBeVisible()
})

test('pin, a section named in place, and dragging pins into it in order', async ({
  page,
}, testInfo) => {
  await resetProjects()
  const tidepool = await projectNamed('tidepool')
  const notes = await projectNamed('notes-site')
  await openProjects(page)
  await expect(pinnedCard(page)).toContainText(
    'Drag the projects you use most here',
  )

  await projectMenu(page, tidepool.id, 'tidepool', 'Pin')
  await expect(toast(page, 'Pinned “tidepool”')).toBeVisible()
  await expect(projectRow(pinnedCard(page), tidepool.id)).toBeVisible()

  // A new section is named in place.
  await pinnedCard(page)
    .getByRole('button', { name: 'Section', exact: true })
    .click()
  const name = pinnedCard(page).getByRole('textbox', { name: 'Section name' })
  await expect(name).toBeFocused()
  await name.fill('Rust work')
  await name.press('Enter')
  const section = pinnedCard(page).locator('[data-section-id]').filter({
    hasText: 'Rust work',
  })
  await expect(section).toBeVisible()
  const sectionId = await section.getAttribute('data-section-id')

  // Drag tidepool onto the section: it goes in.
  await projectRow(page, tidepool.id).hover()
  await dragTo(
    page,
    projectRow(pinnedCard(page), tidepool.id).locator('[data-grip]'),
    section,
  )
  await expect(toast(page, 'Moved “tidepool” to “Rust work”')).toBeVisible()
  await expect
    .poll(
      async () =>
        (await wireProjectList()).arrangement.find(
          (node) => node.id === tidepool.id,
        )?.parent,
    )
    .toBe(sectionId)

  // Pin notes-site, then drag it onto the top half of tidepool: before it.
  await projectMenu(page, notes.id, 'notes-site', 'Pin')
  await expect(projectRow(pinnedCard(page), notes.id)).toBeVisible()
  await projectRow(pinnedCard(page), notes.id).hover()
  await dragTo(
    page,
    projectRow(pinnedCard(page), notes.id).locator('[data-grip]'),
    projectRow(pinnedCard(page), tidepool.id),
    0.2,
  )
  await expect(toast(page, 'Moved “notes-site” to “Rust work”')).toBeVisible()
  await expect
    .poll(async () =>
      (await wireProjectList()).arrangement
        .filter((node) => node.parent === sectionId)
        .map((node) => node.id),
    )
    .toEqual([notes.id, tidepool.id])
  const onScreen = await pinnedCard(page)
    .locator('[data-project-id]')
    .evaluateAll((rows) =>
      rows.map((row) => row.getAttribute('data-project-id')),
    )
  expect(onScreen).toEqual([notes.id, tidepool.id])
  await testInfo.attach('two pins in a section, in the order dropped', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('a project’s page is the Records workbench, scoped to it', async ({
  page,
}) => {
  const tidepool = await projectNamed('tidepool')
  const chats = await wireList({ projectId: tidepool.id })
  await openProjects(page)
  await projectRow(page, tidepool.id)
    .getByRole('link', { name: 'tidepool' })
    .click()
  await expect(page).toHaveURL(
    new RegExp(`#/agent/projects/${encodeURIComponent(tidepool.id)}$`),
  )
  await expect(
    page.getByRole('heading', { name: 'tidepool', level: 1 }),
  ).toBeVisible()
  await expect(
    page.getByRole('searchbox', { name: 'Search conversations' }),
  ).toHaveAttribute('placeholder', 'Search this project’s conversations…')
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    chats[0].id,
  )
  await expectLastRow(page, chats.at(-1)!.id)
  await filterButton(page).click()
  await expect(
    page
      .getByRole('dialog')
      .getByRole('button', { name: 'Project', exact: true }),
  ).toHaveCount(0)
  await page.keyboard.press('Escape')

  // Its conversation opens beside the list, on the project's own page.
  await listRows(page).first().click()
  await expect.poll(() => hashParam(page, 'chat')).toBe(chats[0].id)
  expect(page.url()).toContain(`#/agent/projects/${tidepool.id}?`)

  await page.goto('/#/agent/projects/p-no-such-project')
  await unlockIfLocked(page)
  await expect(page.getByText('Can’t find this project')).toBeVisible()
  await page.getByRole('link', { name: 'Back to projects' }).last().click()
  await expect(page).toHaveURL(/#\/agent\/projects$/)
})
