/**
 * Settings › Manage data types (`data-spaces.md` §2, §8) with browsing on
 * and Agent conversations off, as the change project has them (the agent
 * project turns them on after it).
 *
 * Proves:
 * - each kind has its card: its name, why it is kept, the sources found on
 *   this computer (the browsers' names and the Agent tools', as the
 *   backend's presence check reports them), its state, and what the
 *   archive holds in the kind's own unit, as `list_data_kinds` counts it
 *   (browsing's count is the archive's visit total), and its size once
 *   counted;
 * - browsing, the only kind on, can be neither paused nor turned off: both
 *   are disabled before anything is tried, and the card says why. The
 *   backend refuses the same change on its own (`last-active-kind`), and
 *   nothing changed;
 * - Agent conversations' `On` does not turn them on: it opens the preview
 *   that does, and closing it leaves them off;
 * - while Agent conversations are off, Settings lists no page of their own
 *   under Data types, and that page's address says they are off and leads
 *   to Manage data types.
 */
import { expect, test, type Page } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { enableDialog, expectedToolNames } from './support/agent-tools'
import { backend, expectCount } from './support/fixture'

interface KindList {
  kinds: {
    kind: 'browsing' | 'agent-history'
    state: 'on' | 'paused' | 'off'
    archiveCount?: string
    /** Only when asked for (`includeArchiveBytes`). */
    archiveBytes?: string
  }[]
  policyToken: string
}

interface Presence {
  sources: { kind: string; found: boolean; profileId?: string }[]
  profiles: { profileId: string; browserName: string }[]
}

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/**
 * The card's line about the state and what the archive holds, as
 * `kind-card.tsx` writes it once the sizes have come (`row` from
 * `list_data_kinds` with `includeArchiveBytes`). The size is matched as a
 * size, not to the byte: the archive's files can grow a little between the
 * test's read and the card's (a write-ahead log, an index).
 */
function holdsLine(hint: string, row: KindList['kinds'][number]) {
  if (row.archiveCount === undefined) return `${hint}.`
  const count = Number(row.archiveCount)
  const unit = row.kind === 'browsing' ? 'visit' : 'conversation'
  const holds = `${count.toLocaleString('en-US')} ${unit}${count === 1 ? '' : 's'}`
  return row.archiveBytes === undefined
    ? `${hint}. The archive holds ${holds}.`
    : new RegExp(
        `${escape(`${hint}. The archive holds ${holds}, `)}[\\d.,]+ (B|KB|MB|GB|TB)\\.`,
      )
}

const card = (page: Page, name: string) =>
  page.getByRole('region', { name, exact: true })

const states = (page: Page, name: string) =>
  card(page, name).getByRole('radiogroup', { name: `State of ${name}` })

/** Browsers found on this computer, each once, in the backend's order. */
async function browsersFound() {
  const presence = await backend<Presence>('detect_data_sources', {
    request: { kinds: ['browsing'] },
  })
  const found = new Set(
    presence.sources.flatMap((source) =>
      source.kind === 'browsing' && source.found && source.profileId
        ? [source.profileId]
        : [],
    ),
  )
  return [
    ...new Set(
      presence.profiles
        .filter((profile) => found.has(profile.profileId))
        .map((profile) => profile.browserName),
    ),
  ]
}

test('the only kind on cannot be paused or turned off, and says why; Agent goes through the preview', async ({
  page,
}, testInfo) => {
  const before = await backend<KindList>('list_data_kinds', { request: {} })
  // The cards ask for the sizes once drawn; the lines end with them.
  const sized = await backend<KindList>('list_data_kinds', {
    request: { includeArchiveBytes: true },
  })
  const browsingRow = sized.kinds.find((row) => row.kind === 'browsing')!
  const agentRow = sized.kinds.find((row) => row.kind === 'agent-history')!
  expect(browsingRow.state).toBe('on')
  expect(agentRow.state).toBe('off')
  const visits = await backend<{ totalVisits: number }>(
    'load_dashboard_snapshot',
  )
  await expectCount(
    testInfo,
    'browsing count is the archive visit total',
    Number(browsingRow.archiveCount),
    visits.totalVisits,
  )

  await page.goto('/#/settings/kinds')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Manage data types', level: 2 }),
  ).toBeVisible()

  // Browsing: on, and the last kind on.
  const browsing = card(page, 'Browsing history')
  await expect(browsing).toContainText(
    'Chrome keeps 90 days of history, and Safari keeps a year by default.',
  )
  await expect(browsing).toContainText(
    `Sources: ${(await browsersFound()).join(', ')}`,
  )
  await expect(
    states(page, 'Browsing history').getByRole('radio', { name: 'On' }),
  ).toBeChecked()
  for (const state of ['Paused', 'Off']) {
    await expect(
      states(page, 'Browsing history').getByRole('radio', { name: state }),
    ).toBeDisabled()
  }
  await expect(browsing).toContainText(
    'At least one kind of data has to stay on, so “Browsing history” can’t be paused or turned off. Turn on another kind first.',
  )
  await expect(browsing).toContainText(
    holdsLine(
      'Backed up on schedule, and shown in the navigation',
      browsingRow,
    ),
  )

  // Agent conversations: off, with the tools found here as sources.
  const agent = card(page, 'Agent conversations')
  await expect(agent).toContainText(
    `Sources: ${expectedToolNames().join(', ')}`,
  )
  await expect(
    states(page, 'Agent conversations').getByRole('radio', { name: 'Off' }),
  ).toBeChecked()
  await expect(agent).toContainText(
    holdsLine(
      'No more backups, and not shown in the navigation; saved data is kept',
      agentRow,
    ),
  )
  await testInfo.attach('Manage data types', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })

  // The backend refuses the same change by itself.
  let refusal = ''
  try {
    await backend('set_data_kind_state', {
      request: {
        kind: 'browsing',
        state: 'paused',
        expectedPolicyToken: before.policyToken,
      },
    })
  } catch (error) {
    refusal = String(error)
  }
  expect(refusal).toContain('last-active-kind')

  // Agent's "On" opens the preview instead of turning anything on.
  await states(page, 'Agent conversations')
    .getByRole('radio', { name: 'On' })
    .click()
  await expect(page).toHaveURL(/#\/settings\/kinds\?enable=agent$/)
  await expect(enableDialog(page)).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(enableDialog(page)).toBeHidden()
  await expect(
    states(page, 'Agent conversations').getByRole('radio', { name: 'Off' }),
  ).toBeChecked()

  const after = await backend<KindList>('list_data_kinds', { request: {} })
  expect(after.kinds.map((row) => [row.kind, row.state])).toEqual(
    before.kinds.map((row) => [row.kind, row.state]),
  )
  expect(after.policyToken).toBe(before.policyToken)
})

test('while Agent conversations are off, Settings has no page of their own', async ({
  page,
}) => {
  const kinds = await backend<KindList>('list_data_kinds', { request: {} })
  expect(kinds.kinds.find((row) => row.kind === 'agent-history')?.state).toBe(
    'off',
  )
  await page.goto('/#/settings/kinds')
  await unlockIfLocked(page)
  const group = page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('group', { name: 'Data types' })
  await expect(
    group.getByRole('link', { name: 'Manage data types' }),
  ).toBeVisible()
  await expect(
    group.getByRole('link', { name: 'Agent conversations' }),
  ).toHaveCount(0)

  await page.goto('/#/settings/agent')
  await expect(
    page.getByText(
      'Agent conversations are off, so they have no settings here. Turn them on in Manage data types.',
    ),
  ).toBeVisible()
  await page.getByRole('link', { name: 'Manage data types' }).last().click()
  await expect(page).toHaveURL(/#\/settings\/kinds$/)
})
