/**
 * History in the Agent space (`agent-history.md` §3): the list of saved
 * conversations, on the archive the agent projects left (Claude Code,
 * Codex and Pi backed up; one Codex conversation deleted from its folder,
 * so only PathKeep still has it).
 *
 * Proves, against what the backend lists (`list_agent_conversations`):
 * - switching to the space opens Insights, and History is the next page
 *   in the rail; by date, the newest
 *   conversation is the first row, under a heading that counts its whole
 *   day, and the oldest is the last row once the list is scrolled down;
 * - sorting by token usage puts the most tokens first with each row's
 *   tokens ("No record" for the ones without, last) and drops the date
 *   groups; picking date groups again goes back to last activity;
 * - by project, a project with more than three conversations offers the
 *   rest, which opens that project by date as a new history entry; Back
 *   returns to the project groups;
 * - a tool, "Only in PathKeep" and a time range narrow the list to what
 *   the backend lists for them, each with a chip, counted on the Filter
 *   button; "Clear all" removes them; the deleted conversation carries
 *   the only-in-PathKeep mark;
 * - a day in the address (Insights links so) shows that day's
 *   conversations; a range replaces the day;
 * - the project field lists every project (`list_agent_projects`: its
 *   total, its ids) and narrows to one;
 * - ↓ and j move the selection, which opens the reader beside the list;
 *   Esc closes it.
 */
import { expect, test, type Locator } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { TOOL_NAMES } from './support/agent-tools'
import {
  chatList,
  chatRow,
  chip,
  conversationsText,
  expectLastRow,
  filterButton,
  hashParam,
  listRows,
  openHistory,
  pickFilter,
  readerTitle,
  savedChat,
  scrollListTo,
  wireCounts,
  wireList,
  wirePage,
  wireProjects,
} from './support/agent-chats'
import { wireProjectList } from './support/agent-projects'
import { fixture } from './support/fixture'

const compact = (n: number) =>
  new Intl.NumberFormat('en-US', {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(n)

const projectsText = (n: number) => `All ${n} project${n === 1 ? '' : 's'}`

/** The count at the end of a group's heading. */
const countOf = (heading: Locator) => heading.locator(':scope > span').last()

test('History lists, sorts, groups and filters the saved conversations', async ({
  page,
}, testInfo) => {
  // The space opens at Insights; History is next in the rail, by date,
  // newest first.
  await page.goto('/#/browsing/home')
  await unlockIfLocked(page)
  await page.getByRole('button', { name: /^Switch data type\. Now: / }).click()
  await page
    .getByRole('menu', { name: /^Switch data type/ })
    .getByRole('menuitemradio', { name: /Agent conversations/ })
    .click()
  await expect(page).toHaveURL(/#\/agent\/insights$/)
  await page
    .getByRole('navigation', { name: 'Main navigation' })
    .getByRole('link', { name: 'History', exact: true })
    .click()
  await expect(page).toHaveURL(/#\/agent\/chats$/)
  await expect(
    page.getByRole('heading', { name: 'History', level: 1 }),
  ).toBeVisible()

  const all = await wireList()
  const counts = await wireCounts()
  expect(all).toHaveLength(counts.matchingRoots)
  expect(all.length, 'the agent projects saved dozens').toBeGreaterThan(20)
  const byDay = await wirePage({ limit: 50 })
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    all[0].id,
  )
  await expect(
    countOf(chatList(page).getByRole('heading', { level: 3 }).first()),
  ).toHaveText(conversationsText(byDay.groups[0].count))
  await testInfo.attach('History by date', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await expectLastRow(page, all.at(-1)!.id)

  // By token usage: the most first, with its number; no date groups.
  const byTokens = await wireList({ sort: 'tokens' })
  expect(byTokens[0].tokensTotal).not.toBeNull()
  await pickFilter(page, 'Sort', 'Token usage')
  expect(hashParam(page, 'sort')).toBe('tokens')
  expect(hashParam(page, 'group')).toBe('none')
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    byTokens[0].id,
  )
  await expect(listRows(page).first()).toContainText(
    `${compact(byTokens[0].tokensTotal!)} tokens`,
  )
  await expect(chatList(page).getByRole('heading', { level: 3 })).toHaveCount(0)
  await expectLastRow(page, byTokens.at(-1)!.id)
  if (byTokens.at(-1)!.tokensTotal === null)
    await expect(listRows(page).last()).toContainText('No record')
  await testInfo.attach('History by token usage', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await pickFilter(page, 'Group', 'By date')
  expect(hashParam(page, 'sort')).toBeNull()
  expect(hashParam(page, 'group')).toBeNull()
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    all[0].id,
  )

  // By project: the rest of a big project is one click away; Back returns.
  const projects = await wireProjects()
  const big = projects.find(
    (group) => group.project && (group.remaining ?? 0) > 0,
  )
  expect(big, 'a project with more than three conversations').toBeDefined()
  const bigProject = big!.project!
  await pickFilter(page, 'Group', 'By project')
  expect(hashParam(page, 'group')).toBe('project')
  const heading = chatList(page)
    .getByRole('heading', { level: 3 })
    .filter({ has: page.getByText(bigProject.label, { exact: true }) })
  await scrollListTo(page, heading)
  await expect(countOf(heading)).toHaveText(conversationsText(big!.count))
  const more = chatList(page).locator(`button[data-project="${bigProject.id}"]`)
  await scrollListTo(page, more)
  await expect(more).toHaveText(`${big!.remaining} more, only this project →`)
  await more.click()
  await expect.poll(() => hashParam(page, 'project')).toBe(bigProject.id)
  expect(hashParam(page, 'group')).toBeNull()
  await expect(chip(page, bigProject.label)).toBeVisible()
  const inProject = await wireList({ projectId: bigProject.id })
  expect(inProject).toHaveLength(big!.count)
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    inProject[0].id,
  )
  await expectLastRow(page, inProject.at(-1)!.id)
  await page.goBack()
  await expect.poll(() => hashParam(page, 'group')).toBe('project')
  expect(hashParam(page, 'project')).toBeNull()
  await pickFilter(page, 'Group', 'By date')

  // One tool: only its rows, ending where the backend's list ends.
  const pi = await wireList({ harnesses: ['pi'] })
  expect(pi.length, 'the Pi conversations backed up').toBeGreaterThan(0)
  await pickFilter(page, 'Tool', TOOL_NAMES.pi)
  await expect(chip(page, TOOL_NAMES.pi)).toBeVisible()
  await expect(filterButton(page)).toHaveAccessibleName('Filter (1 condition)')
  await expect(listRows(page).first()).toHaveAttribute('data-chat-id', pi[0].id)
  await expectLastRow(page, pi.at(-1)!.id)
  for (const row of await listRows(page).all())
    await expect(row).toContainText(TOOL_NAMES.pi)

  // And only in PathKeep: the conversation deleted from Codex's folder.
  await pickFilter(page, 'Tool', TOOL_NAMES.pi)
  await pickFilter(page, 'Tool', TOOL_NAMES.codex)
  await pickFilter(page, 'Only', 'Only in PathKeep')
  await expect(filterButton(page)).toHaveAccessibleName('Filter (2 conditions)')
  const gone = await wireList({
    harnesses: ['codex'],
    flags: ['onlyInPathKeep'],
  })
  const removed = fixture().agents.removed.find(
    (conversation) => conversation.tool === 'codex',
  )
  expect(removed, 'the conversation deleted from Codex’s folder').toBeDefined()
  expect(gone.map((row) => row.id)).toContain((await savedChat(removed!)).id)
  await expect(listRows(page)).toHaveCount(gone.length)
  for (const row of gone)
    await expect(
      chatRow(page, row.id).getByRole('img', {
        name: 'The tool’s own folder no longer has this conversation; only PathKeep’s archive still keeps it.',
      }),
    ).toBeVisible()
  await testInfo.attach('Codex, only in PathKeep', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.getByRole('button', { name: 'Clear all' }).click()
  await expect(filterButton(page)).toHaveAccessibleName('Filter and sort')
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    all[0].id,
  )

  // A time range: the last seven days.
  const week = await wireList({ range: '7d' })
  expect(week.length, 'conversations in the last seven days').toBeGreaterThan(0)
  await pickFilter(page, 'Time', '7 days')
  await expect(chip(page, '7 days')).toBeVisible()
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    week[0].id,
  )
  await expectLastRow(page, week.at(-1)!.id)
  await chip(page, '7 days')
    .getByRole('button', { name: 'Remove this filter' })
    .click()
  expect(hashParam(page, 'range')).toBeNull()

  // One day from the address; a range then replaces it.
  const dated = (await wirePage({ limit: 200 })).items.filter(
    (item) => item.day,
  )
  const day = dated[Math.floor(dated.length / 2)].day!
  const onDay = await wireList({ day })
  await openHistory(page, `?day=${day}`)
  await expect(filterButton(page)).toHaveAccessibleName('Filter (1 condition)')
  await expect(listRows(page)).toHaveCount(onDay.length)
  await expect(chatList(page).getByRole('heading', { level: 3 })).toHaveCount(1)
  await expect(
    countOf(chatList(page).getByRole('heading', { level: 3 })),
  ).toHaveText(conversationsText(onDay.length))
  await pickFilter(page, 'Time', '30 days')
  expect(hashParam(page, 'day')).toBeNull()
  expect(hashParam(page, 'range')).toBe('30d')

  // The project field lists every project; one narrows the list.
  await openHistory(page)
  const tidepool = projects.find((group) => group.project?.label === 'tidepool')
  expect(tidepool, 'the tidepool project').toBeDefined()
  const catalog = await wireProjectList()
  expect(
    catalog.items.map((card) => card.id),
    'the list groups and the projects name the same project',
  ).toContain(tidepool!.project!.id)
  await filterButton(page).click()
  const field = page
    .getByRole('dialog')
    .getByRole('button', { name: 'Project', exact: true })
  await expect(field).toHaveText(projectsText(catalog.total))
  await field.click()
  await expect(page.getByRole('option')).toHaveCount(catalog.items.length)
  await page.getByPlaceholder('Find a project…').fill('tidepool')
  await page.getByRole('option', { name: /^tidepool/ }).click()
  // Choosing closes the project list and leaves the filter open. Esc goes to
  // the topmost layer, and a list still fading out is one: wait until it
  // has gone, as a person who sees it close would.
  await expect(page.getByRole('dialog')).toHaveCount(1)
  await expect(field).toHaveText('tidepool')
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect
    .poll(() => hashParam(page, 'project'))
    .toBe(tidepool!.project!.id)
  await expect(chip(page, 'tidepool')).toBeVisible()
  const inTidepool = await wireList({ projectId: tidepool!.project!.id })
  expect(inTidepool).toHaveLength(tidepool!.count)
  await expect(listRows(page).first()).toHaveAttribute(
    'data-chat-id',
    inTidepool[0].id,
  )
  await expectLastRow(page, inTidepool.at(-1)!.id)

  // The keyboard moves the selection, and the selection opens the reader.
  await openHistory(page)
  await chatList(page).focus()
  await page.keyboard.press('ArrowDown')
  await expect.poll(() => hashParam(page, 'chat')).toBe(all[0].id)
  await expect(readerTitle(page)).toBeVisible()
  await page.keyboard.press('j')
  await expect.poll(() => hashParam(page, 'chat')).toBe(all[1].id)
  await expect(chatRow(page, all[1].id)).toHaveAttribute('aria-current', 'true')
  await testInfo.attach('the second conversation, open beside the list', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await page.keyboard.press('Escape')
  await expect.poll(() => hashParam(page, 'chat')).toBeNull()
  await expect(page.getByText('Pick a conversation to read')).toBeVisible()
})
