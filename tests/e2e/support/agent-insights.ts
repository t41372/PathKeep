/**
 * The Agent space's Insights, for the agent-insights specs: the report run
 * through the backend the way the window runs one, read to its last frame
 * over the bridge (`/previews/{streamId}/events`), to check the screen
 * against; the saved choices; the page's parts; and the numbers written
 * the way the English screen writes them.
 */
import { expect, type Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { unlockIfLocked } from './app'
import { backend } from './fixture'

export interface WireTokens {
  input: number
  cacheWrite: number
  cacheRead: number
  output: number
}

export interface WireOverview {
  agentBusyMs: number | null
  agentSummedMs: number | null
  presentMs: number | null
  conversationCount: number
  subagentCount: number
  projectCount: number
  tokens: WireTokens | null
  conversationsWithUsage: number
  cost?: {
    totalUsd: number | null
    recordedUsd: number | null
    estimatedUsd: number | null
    availableEstimatedUsd: number | null
    recordedConversations: number
    estimatedConversations: number
    unpricedConversations: number
    includeEstimates: boolean
  }
  savedRoots: number
}

export interface WireCalendar {
  year: number
  currentYear: number
  days: {
    day: string
    busyMs: number | null
    active: boolean
    future: boolean
  }[]
  activeDays: number
  busiestDay: string | null
}

export interface WireBreakdowns {
  longest: {
    conversationId: string
    title: string | null
    agentMsInRange: number
  }[]
}

export interface WireReport {
  started: { range: string; today: string; timeZone: string }
  overview: WireOverview
  breakdowns: WireBreakdowns
  calendar: WireCalendar
}

export interface InsightsPreferences {
  range: 'today' | '7d' | '30d' | '90d' | '1y' | 'all'
  equivalent: 'nonstop' | 'workdays'
  showCost: boolean
  estimateCost: boolean
}

async function wireFrames(request: Record<string, unknown>) {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const started = await backend<{ streamId: string }>('get_agent_insights', {
    request,
  })
  const response = await fetch(
    `${devIpcUrl}/previews/${encodeURIComponent(started.streamId)}/events`,
  )
  const frames = (await response.text())
    .split('\n')
    .filter((line) => line.startsWith('data:'))
    .map(
      (line) =>
        JSON.parse(line.slice('data:'.length)) as {
          type: string
          payload: unknown
        },
    )
  const end = frames.at(-1)
  if (end?.type !== 'finished')
    throw new Error(
      `The report did not finish: ${end?.type} ${JSON.stringify(end?.payload)}`,
    )
  return (type: string) =>
    frames.findLast((frame) => frame.type === type)?.payload
}

/**
 * A whole report (both parts) for `request`, as the backend answers it
 * now: the last section of each kind. Fails when the stream fails.
 */
export async function wireInsights(
  request: Record<string, unknown> = {},
): Promise<WireReport> {
  const last = await wireFrames({ ...request, part: 'both' })
  return {
    started: last('started') as WireReport['started'],
    overview: last('overview') as WireOverview,
    breakdowns: last('breakdowns') as WireBreakdowns,
    calendar: last('calendar') as WireCalendar,
  }
}

/** One year of the calendar for `request` (`year` and the filters). */
export async function wireCalendar(request: Record<string, unknown> = {}) {
  const last = await wireFrames({ ...request, part: 'calendar' })
  return last('calendar') as WireCalendar
}

export function wirePreferences() {
  return backend<InsightsPreferences>('get_agent_insights_preferences', {
    request: {},
  })
}

export function setPreferences(patch: Partial<InsightsPreferences>) {
  return backend<InsightsPreferences>('set_agent_insights_preferences', {
    request: patch,
  })
}

export async function openInsights(page: Page, search = '') {
  await page.goto(`/#/agent/insights${search}`)
  await unlockIfLocked(page)
  await expect(insightsHeading(page)).toBeVisible()
}

export const insightsHeading = (page: Page) =>
  page.getByRole('heading', { name: /^Insights/, level: 1 })

/** A number under the title, by its label (not the conversations tile). */
export const kpi = (page: Page, label: string) =>
  page.getByRole('group', { name: label, exact: true })

/** The conversations tile, which is a link into History. */
export const conversationsTile = (page: Page) =>
  page.getByRole('link', {
    name: /^\d[\d,]* conversations?: open (it|them) in History$/,
  })

export const conversationsName = (n: number) =>
  n === 1
    ? '1 conversation: open it in History'
    : `${n.toLocaleString('en-US')} conversations: open them in History`

export const rangeToggle = (page: Page) =>
  page.getByRole('radiogroup', { name: 'Time range' })

export const filterButton = (page: Page) =>
  page.getByRole('button', {
    name: /^(Filter tools and projects|Filter \(\d+ conditions?\))$/,
  })

/** Opens the filter, presses a tool, closes it. */
export async function pickTool(page: Page, tool: string) {
  await filterButton(page).click()
  await page
    .getByRole('dialog')
    .getByRole('group', { name: 'Tools', exact: true })
    .getByRole('button', { name: tool, exact: true })
    .click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toHaveCount(0)
}

/** "45 min", "3.5 h", "12 h": Agent time as the screen writes it. */
export function hoursText(ms: number) {
  const minutes = ms / 60_000
  if (minutes < 60) return `${Math.round(minutes)} min`
  const hours = minutes / 60
  const value = hours < 10 ? Math.round(hours * 10) / 10 : Math.round(hours)
  return `${value.toLocaleString('en-US')} h`
}

export const compact = (n: number) =>
  new Intl.NumberFormat('en-US', {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(n)

export const usd = (value: number) =>
  new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: value > 0 && value < 1 ? 4 : 2,
  }).format(value)

export const tokenSum = (tokens: WireTokens) =>
  tokens.input + tokens.cacheWrite + tokens.cacheRead + tokens.output

/** A calendar cell's name: its day and the Agent work on it. */
export function dayCell(day: WireCalendar['days'][number]) {
  if (day.active && day.busyMs === null)
    return `${day.day} · conversations, but no time on record`
  if (!day.busyMs) return `${day.day} · none`
  return `${day.day} · Agent work ${hoursText(day.busyMs)}`
}

export function coverageText(known: number, count: number) {
  return count === 1
    ? `${known} of 1 conversation has a usage record`
    : `${known} of ${count} conversations have a usage record`
}

/** Steps the year calendar back until it shows `year`. */
export async function showYear(page: Page, year: number) {
  const grid = (y: number) =>
    page.getByRole('group', { name: `Agent work per day in ${y}` })
  for (let step = 0; step < 5; step += 1) {
    if (await grid(year).isVisible()) return
    await page.getByRole('button', { name: 'Previous year' }).click()
  }
  await expect(grid(year)).toBeVisible()
}

/** A search parameter of the hash route (`#/agent/chats?day=…`). */
export function hashParams(page: Page) {
  const hash = new URL(page.url()).hash
  const at = hash.indexOf('?')
  return new URLSearchParams(at < 0 ? '' : hash.slice(at + 1))
}
