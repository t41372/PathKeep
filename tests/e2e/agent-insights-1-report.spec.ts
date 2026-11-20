/**
 * Insights in the Agent space (`agent-history.md` §2), on the archive the
 * agent projects left (Claude Code, Codex and Pi backed up). Every number
 * is checked against what the backend's report says for the same choices
 * (`get_agent_insights`), never typed in.
 *
 * Proves:
 * - switching to the space opens Insights, its first page, on the saved
 *   range: the five numbers and the year are the backend's;
 * - a range is a saved choice: 30 days reads the 30-day report, is kept
 *   in the preferences and is still chosen after a reload; "Today", which
 *   holds no conversation in the fixture, says so in words (no zeros) and
 *   keeps the year, and "Show all time" goes back;
 * - the cost switch on the cost card is the saved setting: off, the total
 *   is only what the tools recorded and the estimate is struck through;
 *   "Show cost" off (Settings' switch, set here through the backend)
 *   takes the cost tile and card away at once;
 * - a tool filter narrows the numbers and the year, and the drill-ins
 *   carry it: the conversations tile opens History with the tool, a day
 *   of the year opens that day's conversations of the tool, and a longest
 *   conversation opens in the reader.
 * The saved choices are put back as they were.
 */
import { expect, test, type Page } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { chatList, chatRow, readerTitle, wireList } from './support/agent-chats'
import {
  compact,
  conversationsName,
  conversationsTile,
  coverageText,
  dayCell,
  hashParams,
  hoursText,
  insightsHeading,
  kpi,
  openInsights,
  pickTool,
  rangeToggle,
  setPreferences,
  showYear,
  tokenSum,
  usd,
  wireCalendar,
  wireInsights,
  wirePreferences,
  type WireOverview,
} from './support/agent-insights'

async function expectNumbers(page: Page, overview: WireOverview) {
  await expect(conversationsTile(page)).toHaveAccessibleName(
    conversationsName(overview.conversationCount),
  )
  await expect(kpi(page, 'Agent work time')).toContainText(
    overview.agentBusyMs === null ? '—' : hoursText(overview.agentBusyMs),
  )
  await expect(kpi(page, 'Time you were there')).toContainText(
    overview.presentMs === null ? '—' : hoursText(overview.presentMs),
  )
  const tokens = kpi(page, 'Tokens')
  if (overview.tokens === null) await expect(tokens).toContainText('—')
  else await expect(tokens).toContainText(compact(tokenSum(overview.tokens)))
  if (overview.conversationsWithUsage < overview.conversationCount)
    await expect(tokens).toContainText(
      coverageText(overview.conversationsWithUsage, overview.conversationCount),
    )
  else await expect(tokens).toContainText('of them are cache reads')
  const cost = overview.cost
  if (cost)
    await expect(kpi(page, 'Cost (at API prices)')).toContainText(
      cost.totalUsd === null ? '—' : usd(cost.totalUsd),
    )
}

test('Insights open the Agent space and say what the backend counts', async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000)
  const before = await wirePreferences()
  try {
    await setPreferences({
      range: 'all',
      equivalent: 'nonstop',
      showCost: true,
      estimateCost: true,
    })

    // Switching to the space opens Insights, on the saved range.
    await page.goto('/#/browsing/home')
    await unlockIfLocked(page)
    await page
      .getByRole('button', { name: /^Switch data type\. Now: / })
      .click()
    await page
      .getByRole('menu', { name: /^Switch data type/ })
      .getByRole('menuitemradio', { name: /Agent conversations/ })
      .click()
    await expect(page).toHaveURL(/#\/agent\/insights$/)
    await expect(insightsHeading(page)).toHaveText('Insights(All time)')
    await expect(
      rangeToggle(page).getByRole('radio', { name: 'All time' }),
    ).toBeChecked()

    const all = await wireInsights({ range: 'all' })
    expect(
      all.overview.conversationCount,
      'the agent projects saved dozens',
    ).toBeGreaterThan(20)
    await expectNumbers(page, all.overview)
    await expect(conversationsTile(page)).toContainText(
      `${all.overview.subagentCount === 1 ? '1 subagent' : `${all.overview.subagentCount} subagents`}`,
    )
    // The year: this year's days with conversations, and the busiest day's cell.
    const calendar = all.calendar
    await expect(
      page.getByRole('group', {
        name: `Agent work per day in ${calendar.year}`,
      }),
    ).toBeVisible()
    await expect(
      page.getByText('Days with conversations', { exact: true }).locator('..'),
    ).toContainText(`${calendar.activeDays} day`)
    if (calendar.busiestDay) {
      const busiest = calendar.days.find((d) => d.day === calendar.busiestDay)!
      await expect(
        page.getByRole('button', { name: dayCell(busiest), exact: true }),
      ).toHaveCount(1)
    }
    await testInfo.attach('Insights, all time', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    // 30 days: the 30-day report, saved, and still chosen after a reload.
    await rangeToggle(page).getByRole('radio', { name: '30 days' }).click()
    await expect(insightsHeading(page)).toHaveText('Insights(Last 30 days)')
    await expect.poll(async () => (await wirePreferences()).range).toBe('30d')
    const month = await wireInsights({ range: '30d' })
    await expectNumbers(page, month.overview)
    await page.reload()
    await unlockIfLocked(page)
    await expect(
      rangeToggle(page).getByRole('radio', { name: '30 days' }),
    ).toBeChecked()
    await expectNumbers(page, month.overview)

    // Today holds no conversation in the fixture: words, not zeros, and the year stays.
    const today = await wireInsights({ range: 'today' })
    await rangeToggle(page).getByRole('radio', { name: 'Today' }).click()
    await expect(insightsHeading(page)).toHaveText('Insights(Today)')
    if (today.overview.conversationCount === 0) {
      await expect(
        page.getByText('No conversation had a turn today.'),
      ).toBeVisible()
      await expect(kpi(page, 'Agent work time')).toHaveCount(0)
      await expect(
        page.getByRole('group', { name: /^Agent work per day in \d{4}$/ }),
      ).toBeVisible()
      await testInfo.attach('Insights, today with nothing in it', {
        body: await page.screenshot(),
        contentType: 'image/png',
      })
      await page.getByRole('button', { name: 'Show all time' }).click()
      await expect(insightsHeading(page)).toHaveText('Insights(All time)')
    } else {
      await expectNumbers(page, today.overview)
      await rangeToggle(page).getByRole('radio', { name: 'All time' }).click()
    }
    await expect.poll(async () => (await wirePreferences()).range).toBe('all')
    await expectNumbers(page, all.overview)

    // The cost switch is the saved setting; off, only what the tools recorded.
    const estimate = page.getByRole('switch', {
      name: /^Estimate conversations without a record at list prices/,
    })
    await expect(estimate).toBeChecked()
    await estimate.click()
    await expect
      .poll(async () => (await wirePreferences()).estimateCost)
      .toBe(false)
    const recorded = (await wireInsights({ range: 'all' })).overview.cost!
    expect(recorded.includeEstimates).toBe(false)
    const costTile = kpi(page, 'Cost (at API prices)')
    await expect(costTile).toContainText(
      recorded.totalUsd === null ? '—' : usd(recorded.totalUsd),
    )
    await expect(costTile).toContainText(
      `Only what the tools recorded (${recorded.recordedConversations} conversation`,
    )
    await expect(page.getByText('Estimated at list prices')).toHaveClass(
      /line-through/,
    )
    await testInfo.attach('cost without estimates', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })
    await estimate.click()
    await expect
      .poll(async () => (await wirePreferences()).estimateCost)
      .toBe(true)
    await expect(costTile).toContainText(/Recorded .* · estimated /)

    // "Show cost" off (Settings' switch): no cost tile, no cost card.
    await setPreferences({ showCost: false })
    await expect(costTile).toHaveCount(0)
    await expect(
      page.getByRole('heading', { name: 'Cost (at API prices)' }),
    ).toHaveCount(0)
    await setPreferences({ showCost: true })
    await expect(costTile).toBeVisible()

    // One tool: the numbers, then the drill-ins carry it.
    await pickTool(page, 'Codex')
    const codex = await wireInsights({ range: 'all', harnesses: ['codex'] })
    await expectNumbers(page, codex.overview)
    await expect(
      page.getByRole('button', { name: 'Filter (1 condition)' }),
    ).toContainText('Codex')

    // The busiest Codex day of this year, or of last year early in January.
    let year = codex.calendar
    if (!year.busiestDay)
      year = await wireCalendar({ year: year.year - 1, harnesses: ['codex'] })
    const busiestDay = year.busiestDay
    expect(busiestDay, 'Codex worked on some day').not.toBeNull()
    await showYear(page, year.year)
    const cell = year.days.find((d) => d.day === busiestDay)!
    await page.getByRole('button', { name: dayCell(cell), exact: true }).click()
    await expect(page).toHaveURL(/#\/agent\/chats\?/)
    expect(hashParams(page).get('day')).toBe(busiestDay)
    expect(hashParams(page).get('h')).toBe('codex')
    const onDay = await wireList({ day: busiestDay, harnesses: ['codex'] })
    expect(onDay.length).toBeGreaterThan(0)
    await expect(chatList(page)).toBeVisible()
    for (const row of onDay.slice(0, 5))
      await expect(chatRow(page, row.id)).toBeVisible()
    await page.goBack()
    await expect(insightsHeading(page)).toBeVisible()

    // Back on Insights the tool is no longer chosen: filters are not kept.
    await expect(
      page.getByRole('button', { name: 'Filter tools and projects' }),
    ).toBeVisible()
    await pickTool(page, 'Codex')
    await conversationsTile(page).click()
    await expect(page).toHaveURL(/#\/agent\/chats\?h=codex$/)
    await page.goBack()
    await expect(insightsHeading(page)).toBeVisible()

    // The longest conversation opens beside History.
    const [longest] = all.breakdowns.longest
    expect(longest, 'some conversation has a time').toBeDefined()
    await openInsights(page)
    const card = page
      .getByRole('heading', { name: 'Longest conversations' })
      .locator('xpath=ancestor::section[1]')
    await expect(card.getByRole('link').first()).toContainText(
      hoursText(longest.agentMsInRange),
    )
    await card.getByRole('link').first().click()
    await expect(page).toHaveURL(/#\/agent\/chats\?chat=/)
    expect(hashParams(page).get('chat')).toBe(longest.conversationId)
    await expect(readerTitle(page)).toBeVisible()
  } finally {
    await setPreferences(before)
  }
})
