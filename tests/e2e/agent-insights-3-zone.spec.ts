/**
 * Agent Insights put days in the time zone browsing Insights show
 * (`agent-history.md` §1.2, §1.3; `insights-time-zone.md`). A conversation
 * held late in the evening in one zone happens the next morning in a zone
 * eight or more hours ahead, so the day it belongs to moves.
 *
 * Proves, for one fixture conversation whose whole span falls on one day
 * in each zone but on different days in the two:
 * - in the zone Insights started in, the year's cell for its day says the
 *   backend's Agent work for that day, and opens History on that day with
 *   the conversation in it;
 * - after choosing the other zone in Settings (and Insights being built in
 *   it), the cell of the other day says the backend's work for it, and
 *   opens History on that day with the conversation; the first day no
 *   longer holds it.
 * Ends in the zone it found, for the specs after it.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AgentConversation } from '../../scripts/fixtures/synthetic-agents.mjs'
import {
  chatList,
  chatRow,
  savedChat,
  wireList,
  type WireChat,
} from './support/agent-chats'
import {
  dayCell,
  hashParams,
  openInsights,
  showYear,
  wireCalendar,
} from './support/agent-insights'
import { backend, fixture } from './support/fixture'

const REBUILD_TIMEOUT = 120_000

function dayIn(ms: number, zone: string) {
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: zone,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(ms)
}

async function shownZone() {
  return (await backend<{ shown: string }>('get_insights_time_zone', {})).shown
}

/** Chooses a zone in Settings › General and waits for Insights to be built in it. */
async function chooseZone(page: Page, zone: string) {
  const place =
    zone === 'UTC' ? 'UTC' : zone.split('/').at(-1)!.replace(/_/g, ' ')
  await page.goto('/#/settings/general')
  await page.getByRole('combobox', { name: 'Insights time zone' }).click()
  await page.getByPlaceholder('Search for a city or zone').fill(place)
  await page
    .getByRole('option', { name: new RegExp(`^${place}`) })
    .first()
    .click()
  await expect
    .poll(
      async () => {
        const state = await backend<{
          shown: string
          next: string
          rebuildingFrom: string | null
        }>('get_insights_time_zone')
        return (
          state.shown === zone &&
          state.next === zone &&
          state.rebuildingFrom === null
        )
      },
      { timeout: REBUILD_TIMEOUT },
    )
    .toBe(true)
}

/** The year's cell for `day` says the backend's work, and opens History there with `id`. */
async function openDay(page: Page, day: string, id: string) {
  const year = await wireCalendar({ year: Number(day.slice(0, 4)) })
  const cell = year.days.find((d) => d.day === day)!
  expect(cell.active, `${day} has a conversation`).toBe(true)
  await openInsights(page)
  await showYear(page, year.year)
  await page.getByRole('button', { name: dayCell(cell), exact: true }).click()
  await expect(page).toHaveURL(/#\/agent\/chats\?day=/)
  expect(hashParams(page).get('day')).toBe(day)
  await expect(chatList(page)).toBeVisible()
  await expect(chatRow(page, id)).toBeVisible()
}

test('the Insights time zone moves a late conversation to the next day', async ({
  page,
}, testInfo) => {
  test.setTimeout(600_000)
  const first = await shownZone()
  const second = first === 'Asia/Taipei' ? 'America/Los_Angeles' : 'Asia/Taipei'
  // Saved, without subagents (their records can run past the parent's
  // last one), from its first record to its last on one day in each zone.
  const candidates = fixture().agents.conversations.filter((c) => {
    if (c.subagents.length > 0) return false
    const a = dayIn(c.startedAt, first)
    const b = dayIn(c.startedAt, second)
    return (
      a === dayIn(c.updatedAt, first) &&
      b === dayIn(c.updatedAt, second) &&
      a !== b
    )
  })
  let late: AgentConversation | undefined
  let saved: WireChat | undefined
  for (const candidate of candidates) {
    saved = await savedChat(candidate).catch(() => undefined)
    if (saved) {
      late = candidate
      break
    }
  }
  expect(
    saved,
    'a saved conversation on another day in the other zone',
  ).toBeDefined()
  const dayFirst = dayIn(late!.startedAt, first)
  const daySecond = dayIn(late!.startedAt, second)

  try {
    await openDay(page, dayFirst, saved!.id)
    await testInfo.attach(`History on ${dayFirst} in ${first}`, {
      body: await page.screenshot(),
      contentType: 'image/png',
    })

    await chooseZone(page, second)
    await openDay(page, daySecond, saved!.id)
    await testInfo.attach(`History on ${daySecond} in ${second}`, {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    // The first day, read in this zone, no longer holds it.
    const firstDay = await wireList({ day: dayFirst })
    expect(firstDay.map((row) => row.id)).not.toContain(saved!.id)
  } finally {
    if ((await shownZone()) !== first) await chooseZone(page, first)
  }
})
