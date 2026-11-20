/**
 * Settings › Agent conversations (`#/settings/agent`, `agent-history.md`
 * §7) on the conversations the projects before it backed up.
 *
 * Proves:
 * - the page is listed under Data types while the kind is on, opens by
 *   pointing at the backup page, and shows what the backend saved: the
 *   scratch places (the home folder itself as `~`), the folders waiting to
 *   be confirmed (their count, and Review opens them), the reader and AI
 *   switches, the cost switches and the price table's version;
 * - "Fold tool calls" off: a conversation opens with every group of tool
 *   calls unfolded, and its HTML export writes them open; back on, both
 *   fold again. (The fixture records no thinking, so "Show thinking" is
 *   checked as a saved choice that survives a reload.)
 * - "Show costs" off: the reader's info panel shows no cost, the estimate
 *   switch can't be changed, and Insights' cost card has the same choice;
 *   the estimate switch off takes away the estimate of a conversation that
 *   has none recorded, and Insights' switch follows it;
 * - scratch places regroup projects: with every place above a project
 *   folder removed it is a project of its own; adding the folder (typed:
 *   a relative path is refused, the same place twice is refused) moves its
 *   conversations into "Loose folders";
 * - the three AI switches are saved one by one, survive a reload, and the
 *   note that nothing is sent anywhere shows only while all three are off.
 *
 * Puts every preference back as it found it, through the backend, even
 * when a step fails.
 */
import { readFileSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import {
  agentChat,
  exportConversation,
  openChat,
  reader,
  savedChat,
  transcript,
  wireList,
} from './support/agent-chats'
import { unlockIfLocked } from './support/app'
import { backend, fixture } from './support/fixture'

interface ViewerPrefs {
  scratchLocations: string[]
  readerCollapseToolCalls: boolean
  readerShowThinking: boolean
  assistantMayRead: boolean
  buildSemanticIndex: boolean
  mcpMayRead: boolean
  priceTableVersion: string
}

interface CostPrefs {
  showCost: boolean
  estimateCost: boolean
}

interface ProjectList {
  items: {
    id: string
    kind: 'repo' | 'folder' | 'scratch'
    mainFolder: string | null
    conversationCount: number
  }[]
  pendingSuggestions: number
  homeFolder: string | null
}

const viewer = () =>
  backend<ViewerPrefs>('get_agent_viewer_preferences', { request: {} })
const costs = () =>
  backend<CostPrefs>('get_agent_insights_preferences', { request: {} })
const projects = () =>
  backend<ProjectList>('list_agent_projects', { request: {} })

let savedViewer: ViewerPrefs
let savedCosts: CostPrefs

test.beforeAll(async () => {
  savedViewer = await viewer()
  savedCosts = await costs()
})

test.afterAll(async () => {
  await backend('set_agent_viewer_preferences', {
    request: {
      scratchLocations: savedViewer.scratchLocations,
      readerCollapseToolCalls: savedViewer.readerCollapseToolCalls,
      readerShowThinking: savedViewer.readerShowThinking,
      assistantMayRead: savedViewer.assistantMayRead,
      buildSemanticIndex: savedViewer.buildSemanticIndex,
      mcpMayRead: savedViewer.mcpMayRead,
    },
  })
  await backend('set_agent_insights_preferences', {
    request: {
      showCost: savedCosts.showCost,
      estimateCost: savedCosts.estimateCost,
    },
  })
})

async function openSettings(page: Page) {
  await page.goto('/#/settings/agent')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Agent conversations', level: 2 }),
  ).toBeVisible()
}

const settingSwitch = (page: Page, name: string) =>
  page.getByRole('switch', { name, exact: true })

/** `path` as the page writes it: the home folder as `~`. */
function placeLabel(place: string, home: string | null) {
  if (!home) return place
  const base = home.replace(/[\\/]+$/, '')
  if (place === base) return '~ (the home folder itself)'
  return place.startsWith(`${base}/`) ? `~${place.slice(base.length)}` : place
}

/** Whether `place` is `folder` or a folder above it, by whole components. */
function covers(place: string, folder: string) {
  const base = place.replace(/[\\/]+$/, '')
  return folder === base || folder.startsWith(`${base}/`)
}

test('the page lists what the backend saved, and is listed while the kind is on', async ({
  page,
}, testInfo) => {
  await page.goto('/#/settings/general')
  await unlockIfLocked(page)
  const nav = page.getByRole('navigation', { name: 'Settings sections' })
  await nav
    .getByRole('group', { name: 'Data types' })
    .getByRole('link', { name: 'Agent conversations' })
    .click()
  await expect(page).toHaveURL(/#\/settings\/agent$/)
  await expect(
    page.getByText(
      'Which tools to back up, other folders to add, and whether a backup reads .git are on the backup page of Agent conversations, not here.',
    ),
  ).toBeVisible()

  const prefs = await viewer()
  const list = await projects()
  const chips = page.getByRole('list', {
    name: 'Scratch folders go to “Loose folders”',
  })
  await expect(chips.getByRole('listitem')).toHaveText(
    prefs.scratchLocations.map((place) => placeLabel(place, list.homeFolder)),
  )
  if (list.homeFolder && prefs.scratchLocations.includes(list.homeFolder))
    await expect(
      chips.getByRole('button', {
        name: 'Remove ~ (the home folder itself)',
      }),
    ).toBeVisible()

  const waiting = list.pendingSuggestions
  await expect(
    page.getByText(
      waiting === 0
        ? 'No folders are waiting for you to confirm.'
        : waiting === 1
          ? '1 folder looks like it belongs to the same project as another'
          : `${waiting} folders look like they belong to the same project as another`,
    ),
  ).toBeVisible()
  await expect(page.getByRole('link', { name: 'Review' })).toHaveCount(
    waiting === 0 ? 0 : 1,
  )

  await expect(settingSwitch(page, 'Fold tool calls')).toHaveAttribute(
    'aria-checked',
    String(prefs.readerCollapseToolCalls),
  )
  await expect(settingSwitch(page, 'Show thinking')).toHaveAttribute(
    'aria-checked',
    String(prefs.readerShowThinking),
  )
  const cost = await costs()
  await expect(settingSwitch(page, 'Show costs')).toHaveAttribute(
    'aria-checked',
    String(cost.showCost),
  )
  await expect(
    page.getByText(
      `(version ${prefs.priceTableVersion}, updated with the app)`,
    ),
  ).toBeVisible()
  await testInfo.attach('Settings › Agent conversations', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  if (waiting > 0) {
    await page.getByRole('link', { name: 'Review' }).click()
    await expect(page).toHaveURL(/#\/agent\/projects\?review=1$/)
    await expect(
      page.getByRole('dialog', {
        name: 'Folders that may belong to the same project',
      }),
    ).toBeVisible()
  }
})

test('the reader’s tool calls follow “Fold tool calls”, and so does the HTML export', async ({
  page,
}, testInfo) => {
  const conversation = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(conversation)
  const groups = () =>
    transcript(page).getByRole('button', { name: /^Used \d+ tools?/ })

  await backend('set_agent_viewer_preferences', {
    request: { readerCollapseToolCalls: true },
  })
  await openChat(page, chat.id)
  await expect(groups().first()).toHaveAttribute('aria-expanded', 'false')
  const folded = readFileSync(
    (await exportConversation(page, /^HTML/, 'html')).path,
    'utf8',
  )
  const calls = (folded.match(/<details><summary>/g) ?? []).length
  expect(calls, 'tool calls in the export').toBeGreaterThan(0)
  expect(folded).not.toContain('<details open>')

  await openSettings(page)
  await settingSwitch(page, 'Fold tool calls').click()
  await expect(settingSwitch(page, 'Fold tool calls')).toHaveAttribute(
    'aria-checked',
    'false',
  )
  await expect
    .poll(async () => (await viewer()).readerCollapseToolCalls)
    .toBe(false)

  await openChat(page, chat.id)
  await expect(groups().first()).toHaveAttribute('aria-expanded', 'true')
  for (const group of await groups().all())
    await expect(group).toHaveAttribute('aria-expanded', 'true')
  await testInfo.attach('tool calls unfolded', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  const open = readFileSync(
    (await exportConversation(page, /^HTML/, 'html')).path,
    'utf8',
  )
  expect((open.match(/<details open><summary>/g) ?? []).length).toBe(calls)
  expect(open).not.toContain('<details><summary>')

  // "Show thinking": saved, and still so after a reload.
  await openSettings(page)
  const thinking = settingSwitch(page, 'Show thinking')
  const before = (await viewer()).readerShowThinking
  await thinking.click()
  await expect
    .poll(async () => (await viewer()).readerShowThinking)
    .toBe(!before)
  await page.reload()
  await unlockIfLocked(page)
  await expect(settingSwitch(page, 'Show thinking')).toHaveAttribute(
    'aria-checked',
    String(!before),
  )

  // Back on: folded again.
  await settingSwitch(page, 'Fold tool calls').click()
  await expect
    .poll(async () => (await viewer()).readerCollapseToolCalls)
    .toBe(true)
  await openChat(page, chat.id)
  await expect(groups().first()).toHaveAttribute('aria-expanded', 'false')
})

test('the cost switches: what the reader shows, and the same choice in Insights', async ({
  page,
}, testInfo) => {
  await backend('set_agent_insights_preferences', {
    request: { showCost: true, estimateCost: true },
  })
  const pi = await savedChat(
    agentChat(fixture().agents.scenes.piUsage.conversation),
  )
  // A conversation with no recorded cost that the price table can estimate.
  const estimated = await (async () => {
    for (const row of await wireList({ harnesses: ['codex'] })) {
      const info = await backend<{
        total?: { estimatedUsd?: number | null; recordedUsd?: number | null }
      }>('get_agent_conversation_info', {
        request: { id: row.id, showCost: true, estimateCost: true },
      })
      if ((info.total?.estimatedUsd ?? 0) > 0 && !info.total?.recordedUsd)
        return row
    }
    return null
  })()
  expect(estimated, 'a conversation with only an estimated cost').not.toBe(null)

  const panel = () =>
    page.getByRole('complementary', { name: 'About this conversation' })
  const openInfo = async (id: string) => {
    await openChat(page, id)
    await reader(page)
      .getByRole('button', { name: 'Info', exact: true })
      .click()
    await expect(panel()).toBeVisible()
  }

  await openInfo(pi.id)
  await expect(panel()).toContainText('Pi recorded this amount at API prices')
  await openInfo(estimated!.id)
  await expect(panel()).toContainText(
    'Estimated with the price table that comes with PathKeep',
  )

  // Estimate off: the recorded cost stays, the estimate goes.
  await openSettings(page)
  const estimate = settingSwitch(
    page,
    'Estimate conversations without a record at list prices',
  )
  await estimate.click()
  await expect.poll(async () => (await costs()).estimateCost).toBe(false)
  await openInfo(estimated!.id)
  await expect(panel()).not.toContainText(
    'Estimated with the price table that comes with PathKeep',
  )
  await openInfo(pi.id)
  await expect(panel()).toContainText('Pi recorded this amount at API prices')

  // Insights' cost card has the same switch, and it is off too.
  await page.goto('/#/agent/insights')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('switch', {
      name: /^Estimate conversations without a record at list prices/,
    }),
  ).toHaveAttribute('aria-checked', 'false')

  // Show costs off: no cost at all, and the estimate can't be changed.
  await openSettings(page)
  await settingSwitch(page, 'Show costs').click()
  await expect.poll(async () => (await costs()).showCost).toBe(false)
  await expect(estimate).toBeDisabled()
  await expect(
    page.getByText('Turn on “Show costs” to choose this.'),
  ).toBeVisible()
  await testInfo.attach('costs off', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
  await openInfo(pi.id)
  await expect(panel()).not.toContainText('Pi recorded this amount')
  await expect(panel()).not.toContainText('Estimated with the price table')

  await openSettings(page)
  await settingSwitch(page, 'Show costs').click()
  await expect(estimate).toBeEnabled()
  await estimate.click()
  await expect
    .poll(async () => await costs())
    .toEqual(expect.objectContaining({ showCost: true, estimateCost: true }))
})

test('scratch places regroup a project folder into “Loose folders” and back', async ({
  page,
}, testInfo) => {
  const codex = fixture().agents.conversations.filter(
    (conversation) => conversation.tool === 'codex',
  )
  const folder = codex.find((conversation) =>
    conversation.projectFolder.endsWith('/tidepool'),
  )?.projectFolder
  expect(folder, 'Codex worked in tidepool').toBeDefined()
  const before = await viewer()
  const home = (await projects()).homeFolder
  // The home folder counts only itself, not what is inside it.
  const above = before.scratchLocations.filter(
    (place) => place !== home && covers(place, folder!),
  )

  await openSettings(page)
  const chips = page.getByRole('list', {
    name: 'Scratch folders go to “Loose folders”',
  })
  // Every place above the folder (the system's temporary folder holds the
  // test's files) comes off first, so the folder is a project of its own.
  for (const place of above) {
    await chips
      .getByRole('button', { name: `Remove ${placeLabel(place, home)}` })
      .click()
    await expect
      .poll(async () => (await viewer()).scratchLocations)
      .not.toContain(place)
  }
  await expect
    .poll(async () =>
      (await projects()).items.some((item) => item.mainFolder === folder),
    )
    .toBe(true)
  const own = await projects()
  const project = own.items.find((item) => item.mainFolder === folder)!
  expect(project.kind).toBe('folder')
  const loose = (list: ProjectList) =>
    list.items.find((item) => item.kind === 'scratch')?.conversationCount ?? 0
  const looseBefore = loose(own)

  // A relative path is refused, and so is a place already in the list.
  await page.getByRole('button', { name: 'Add place' }).click()
  const form = page.getByRole('form', { name: 'Add a scratch folder' })
  const field = form.getByLabel('Folder', { exact: true })
  await field.fill('code/tidepool')
  await form.getByRole('button', { name: 'Add', exact: true }).click()
  await expect(
    form.getByText('Use the full path, starting from the top of the disk.'),
  ).toBeVisible()
  const listed = (await viewer()).scratchLocations[0]
  if (listed) {
    await field.fill(listed)
    await form.getByRole('button', { name: 'Add', exact: true }).click()
    await expect(
      form.getByText('This place is already in the list.'),
    ).toBeVisible()
  }
  await field.fill(`${folder!}/`)
  await form.getByRole('button', { name: 'Add', exact: true }).click()
  await expect(form).toBeHidden()
  await expect(
    chips.getByRole('listitem').filter({ hasText: placeLabel(folder!, home) }),
  ).toBeVisible()
  await expect
    .poll(async () => (await viewer()).scratchLocations)
    .toContain(folder)

  await expect
    .poll(async () =>
      (await projects()).items.some((item) => item.mainFolder === folder),
    )
    .toBe(false)
  expect(loose(await projects())).toBe(looseBefore + project.conversationCount)
  await testInfo.attach('the folder added as a scratch place', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // Taken off again: its own project once more.
  await chips
    .getByRole('button', { name: `Remove ${placeLabel(folder!, home)}` })
    .click()
  await expect
    .poll(
      async () =>
        (await projects()).items.find((item) => item.mainFolder === folder)
          ?.conversationCount,
    )
    .toBe(project.conversationCount)
})

test('the three AI switches are saved one by one', async ({ page }) => {
  await backend('set_agent_viewer_preferences', {
    request: {
      assistantMayRead: false,
      buildSemanticIndex: false,
      mcpMayRead: false,
    },
  })
  await openSettings(page)
  const nothingSent = page.getByText(
    'With all three off, Agent conversations aren’t sent anywhere.',
  )
  await expect(nothingSent).toBeVisible()
  const switches = [
    ['Assistant can read Agent conversations', 'assistantMayRead'],
    ['Build a semantic search index', 'buildSemanticIndex'],
    ['MCP can read Agent conversations', 'mcpMayRead'],
  ] as const
  for (const [name, field] of switches) {
    await settingSwitch(page, name).click()
    await expect.poll(async () => (await viewer())[field]).toBe(true)
    await expect(nothingSent).toHaveCount(0)
    for (const [otherName, other] of switches)
      if (other !== field)
        expect((await viewer())[other], otherName).toBe(
          switches.findIndex(([, f]) => f === other) <
            switches.findIndex(([, f]) => f === field),
        )
  }
  await page.reload()
  await unlockIfLocked(page)
  for (const [name] of switches)
    await expect(settingSwitch(page, name)).toHaveAttribute(
      'aria-checked',
      'true',
    )
  for (const [name, field] of switches) {
    await settingSwitch(page, name).click()
    await expect.poll(async () => (await viewer())[field]).toBe(false)
  }
  await expect(nothingSent).toBeVisible()
})
