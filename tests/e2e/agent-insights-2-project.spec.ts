/**
 * A conversation without a usage record, and Insights of one project
 * (`agent-history.md` §1.11, §2.2, §2.5; the address's `?project=` is how
 * a project's own pages open its Insights).
 *
 * Codex versions before mid-2025 wrote no token counts. Here a new Codex
 * conversation is written the way such a version leaves it (the fixture's
 * own mutator, then its token counts taken out) in a project Codex never
 * worked in, and backed up for real.
 *
 * Proves, against the backend's report for the same choices:
 * - `#/agent/insights?project=<id>` names the project under the title and
 *   counts only its conversations; the token tile says how many of them
 *   have a usage record instead of counting the new one as 0;
 * - with Codex chosen too, the one conversation left has no usage at all:
 *   tokens and cost read "—" with "0 of 1 conversation has a usage
 *   record", the cost card counts it as having no price, and the token mix
 *   says that what has no record isn't counted as 0;
 * - the ✕ beside the project's name drops it from the address and the
 *   numbers go back to the backend's for Codex alone;
 * - an address naming a project that does not exist says so and offers
 *   all projects, instead of a report of nothing.
 * The archive keeps the new conversation; the facts say so for the specs
 * after this one.
 */
import { readFileSync, statSync, utimesSync, writeFileSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import {
  addAgentConversation,
  type SyntheticAgents,
} from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  agentRunWhole,
  count,
  newestRunRef,
  openBackupPage,
  waitForRunAfter,
} from './support/agent-backup'
import {
  conversationsName,
  conversationsTile,
  coverageText,
  hashParams,
  insightsHeading,
  kpi,
  openInsights,
  pickTool,
  setPreferences,
  wireInsights,
  wirePreferences,
  wireProjectCards,
} from './support/agent-insights'
import { fixture, replaceAgents } from './support/fixture'

/** Projects of the fixture Codex never works in (`TOOL_PROJECTS`). */
const NOT_CODEX = ['notes-site', 'ledger']

/**
 * Takes the token counts out of a Codex rollout, as older versions wrote
 * it, and keeps the fixture's facts about the file true.
 */
function withoutTokenCounts(agents: SyntheticAgents, file: string) {
  const kept = readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => {
      if (!line.trim()) return false
      const row = JSON.parse(line) as {
        type?: string
        payload?: { type?: string }
      }
      return !(row.type === 'event_msg' && row.payload?.type === 'token_count')
    })
  const facts = agents.tools.codex.files.find((entry) => entry.path === file)!
  writeFileSync(file, `${kept.join('\n')}\n`)
  utimesSync(file, facts.modifiedAt / 1000, facts.modifiedAt / 1000)
  const bytes = statSync(file).size
  agents.tools.codex.totalBytes += bytes - facts.bytes
  facts.bytes = bytes
}

test('a conversation without usage reads as unknown, and a project in the address filters the report', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  const before = await wirePreferences()
  try {
    await setPreferences({ range: 'all', showCost: true, estimateCost: true })

    // A project with conversations of other tools and none of Codex.
    const cards = await wireProjectCards()
    let agents = fixture().agents
    const target = agents.projects
      .filter((project) => NOT_CODEX.includes(project.key))
      .map((project) => ({
        project,
        card: cards.items.find((card) => card.mainFolder === project.folder),
      }))
      .find(
        ({ card }) =>
          card !== undefined &&
          card.conversationCount > 0 &&
          !card.harnesses.includes('codex'),
      )
    expect(target, 'a saved project Codex never worked in').toBeDefined()
    const card = target!.card!
    const projectId = card.id
    const earlier = (await wireInsights({ range: 'all', projectId })).overview

    // An older Codex's conversation there, backed up for real.
    agents = addAgentConversation(agents, 'codex', {
      project: target!.project.key,
    })
    const added = agents.conversations.at(-1)!
    expect(added.tool).toBe('codex')
    expect(added.files.every((file) => file.endsWith('.jsonl'))).toBe(true)
    for (const file of added.files) withoutTokenCounts(agents, file)
    // The facts have no "unknown": nothing is recorded, so nothing is counted.
    added.usage = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }
    replaceAgents(agents)
    await openBackupPage(page)
    const previous = await newestRunRef()
    await page.getByRole('button', { name: 'Back up now' }).click()
    const run = await waitForRunAfter(previous)
    const detail = await agentRunWhole(run.runRef)
    expect(count(detail.summary.newRoots)).toBe(1)

    // The project's Insights: one conversation more, and no more with usage.
    const inProject = await wireInsights({ range: 'all', projectId })
    expect(inProject.overview.conversationCount).toBe(
      earlier.conversationCount + 1,
    )
    expect(inProject.overview.conversationsWithUsage).toBe(
      earlier.conversationsWithUsage,
    )
    await openInsights(page, `?project=${encodeURIComponent(projectId)}`)
    await expect(insightsHeading(page)).toHaveText('Insights(All time)')
    const line = page.getByText('Only this project:')
    await expect(line).toContainText(card.displayName)
    await expect(conversationsTile(page)).toHaveAccessibleName(
      conversationsName(inProject.overview.conversationCount),
    )
    await expect(kpi(page, 'Tokens')).toContainText(
      coverageText(
        inProject.overview.conversationsWithUsage,
        inProject.overview.conversationCount,
      ),
    )
    await expect(
      page.getByRole('button', { name: 'Filter (1 condition)' }),
    ).toContainText(card.displayName)
    await testInfo.attach('Insights of one project', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // With Codex too, only the conversation without usage is left.
    await pickTool(page, 'Codex')
    const alone = await wireInsights({
      range: 'all',
      projectId,
      harnesses: ['codex'],
    })
    expect(alone.overview.conversationCount).toBe(1)
    expect(alone.overview.tokens).toBeNull()
    expect(alone.overview.cost?.totalUsd ?? null).toBeNull()
    await expect(conversationsTile(page)).toHaveAccessibleName(
      conversationsName(1),
    )
    const tokens = kpi(page, 'Tokens')
    await expect(tokens).toContainText('—')
    await expect(tokens).toContainText(coverageText(0, 1))
    await expect(tokens).not.toContainText(/\b0 tokens?\b/)
    await expect(kpi(page, 'Cost (at API prices)')).toContainText('—')
    const costCard = page
      .getByRole('heading', { name: 'Cost (at API prices)' })
      .locator('xpath=ancestor::section[1]')
    await expect(costCard).toContainText(
      'No price (local model or no usage record)',
    )
    await expect(costCard).toContainText('1 conversation')
    const mix = page
      .getByRole('heading', { name: 'Token mix' })
      .locator('xpath=ancestor::section[1]')
    await expect(mix).toContainText(
      'No conversation in this range has a usage record.',
    )
    await expect(mix).toContainText('0 of 1 conversation has a usage record.')
    await expect(mix).toContainText('Those without a record don’t count as 0.')
    await testInfo.attach('a conversation without usage', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // ✕ beside the name: all projects, Codex still chosen.
    await page.getByRole('button', { name: 'Show all projects' }).click()
    await expect(page.getByText('Only this project:')).toHaveCount(0)
    expect(hashParams(page).get('project')).toBeNull()
    const codex = await wireInsights({ range: 'all', harnesses: ['codex'] })
    await expect(conversationsTile(page)).toHaveAccessibleName(
      conversationsName(codex.overview.conversationCount),
    )
    await expect(kpi(page, 'Tokens')).toContainText(
      coverageText(
        codex.overview.conversationsWithUsage,
        codex.overview.conversationCount,
      ),
    )

    // A project that is not there is said, not reported as nothing.
    await openInsights(page, '?project=no-such-project')
    await expect(page.getByText('Can’t find this project')).toBeVisible()
    await expect(kpi(page, 'Agent work time')).toHaveCount(0)
    await page.getByRole('button', { name: 'Show all projects' }).last().click()
    expect(hashParams(page).get('project')).toBeNull()
    await expect(conversationsTile(page)).toBeVisible()
  } finally {
    await setPreferences(before)
  }
})
