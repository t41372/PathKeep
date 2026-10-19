/**
 * Turns loaded visits (newest first) into the flat row list the virtual
 * timeline renders: day headers, session headers and visit rows.
 *
 * Responsible for: day and session grouping, one row for a page visited
 * several times in a row (the user's choice, 2026-10-12: "×N", each time
 * still reachable), and keeping counts honest while more pages are still to
 * come.
 * Not responsible for: fetching or rendering.
 */
import { startOfDay } from '@/lib/i18n'
import type { VisitItem } from './history-types'

/** Visits closer together than this belong to the same session. */
export const SESSION_GAP_MS = 30 * 60_000

export type TimelineRow =
  | { kind: 'day'; key: string; dayStart: number; count: number | null }
  | {
      kind: 'session'
      key: string
      start: number
      end: number
      count: number | null
      site: string
    }
  | {
      kind: 'visit'
      key: string
      /** The newest visit of the row. */
      item: VisitItem
      /** Every visit the row stands for, newest first, when there is more than one. */
      repeats?: VisitItem[]
    }
  | { kind: 'loading'; key: string }

export interface Timeline {
  rows: TimelineRow[]
  /** Other pages visited in the same session as `id`, newest first. */
  sessionMates: (id: number) => VisitItem[]
  /** Every visit of the row `id` belongs to, when the row stands for several. */
  repeatsOf: (id: number) => VisitItem[]
}

interface Session {
  items: VisitItem[]
}

function topDomain(items: VisitItem[]) {
  const counts = new Map<string, number>()
  for (const item of items)
    counts.set(item.domain, (counts.get(item.domain) ?? 0) + 1)
  let best = items[0].domain
  for (const [domain, count] of counts) {
    if (count > (counts.get(best) ?? 0)) best = domain
  }
  return best
}

/**
 * Only the last group can still grow while `hasMore` is true, so its count is
 * left out until the next page settles it.
 */
export function buildTimeline(items: VisitItem[], hasMore: boolean): Timeline {
  const days: { dayStart: number; sessions: Session[] }[] = []
  const sessionOf = new Map<number, Session>()
  const runOf = new Map<number, VisitItem[]>()

  for (const item of items) {
    const dayStart = startOfDay(item.visitTime).getTime()
    let day = days.at(-1)
    if (!day || day.dayStart !== dayStart) {
      day = { dayStart, sessions: [] }
      days.push(day)
    }
    let session = day.sessions.at(-1)
    const previous = session?.items.at(-1)
    if (
      !session ||
      !previous ||
      previous.visitTime - item.visitTime > SESSION_GAP_MS
    ) {
      session = { items: [] }
      day.sessions.push(session)
    }
    session.items.push(item)
    sessionOf.set(item.id, session)
  }

  const rows: TimelineRow[] = []
  days.forEach((day, dayIndex) => {
    const lastDay = dayIndex === days.length - 1
    const dayOpen = lastDay && hasMore
    rows.push({
      kind: 'day',
      key: `d:${day.dayStart}`,
      dayStart: day.dayStart,
      count: dayOpen
        ? null
        : day.sessions.reduce((sum, s) => sum + s.items.length, 0),
    })
    day.sessions.forEach((session, sessionIndex) => {
      const first = session.items[0]
      const last = session.items[session.items.length - 1]
      const open = dayOpen && sessionIndex === day.sessions.length - 1
      rows.push({
        kind: 'session',
        key: `s:${first.id}`,
        start: last.visitTime,
        end: first.visitTime,
        count: open ? null : session.items.length,
        site: topDomain(session.items),
      })
      // A page visited several times in a row is one row; the session and day
      // still count every visit.
      for (let index = 0; index < session.items.length; ) {
        const item = session.items[index]
        let end = index + 1
        while (
          end < session.items.length &&
          session.items[end].url === item.url
        )
          end += 1
        const repeats =
          end - index > 1 ? session.items.slice(index, end) : undefined
        for (const visit of repeats ?? []) runOf.set(visit.id, repeats!)
        rows.push({ kind: 'visit', key: `v:${item.id}`, item, repeats })
        index = end
      }
    })
  })

  return {
    rows,
    sessionMates: (id) => {
      const run = runOf.get(id)
      return (
        sessionOf
          .get(id)
          ?.items.filter((item) => item.id !== id && !run?.includes(item)) ?? []
      )
    },
    repeatsOf: (id) => runOf.get(id) ?? [],
  }
}

/** Whether `row` stands for the visit `id`, itself or as one of its repeats. */
export function rowHasVisit(row: TimelineRow, id: number) {
  return (
    row.kind === 'visit' &&
    (row.item.id === id ||
      (row.repeats?.some((item) => item.id === id) ?? false))
  )
}

export function visitRows(items: VisitItem[]): TimelineRow[] {
  return items.map((item) => ({ kind: 'visit', key: `v:${item.id}`, item }))
}

export const ROW_HEIGHT = {
  day: 52,
  session: 34,
  visit: 38,
  result: 54,
  loading: 38,
} as const
