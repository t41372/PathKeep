/**
 * The Insights report as it streams in: the KPI row and the wall (the
 * report, which follows the range) and the year (the calendar, which does
 * not). Each section replaces the one of its type as it arrives
 * (`desktop-command-surface.md` §Agent Insights).
 *
 * Responsibilities: asking for only what changed (a range change asks for
 * the report, a year step for the calendar, a filter for both), stopping
 * a request a newer one replaced, keeping the last sections on screen,
 * marked stale, until the new ones arrive, asking again when a backup or
 * a mark changes the conversations, and Retry after a failure.
 * Not responsible for: the choices themselves (`insights-page.tsx`) or
 * drawing anything.
 */
import {
  useCallback,
  useEffect,
  useEffectEvent,
  useReducer,
  useRef,
  useState,
} from 'react'
import type {
  InsightsBreakdowns,
  InsightsCalendar,
  InsightsFrame,
  InsightsOverview,
  InsightsQuery,
  InsightsRange,
  InsightsSeries,
  InsightsWork,
} from '@/lib/backend-client/agent-insights'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { subscribeToAgentConversations } from '@/lib/ipc/agent-chats-events'
import {
  openAgentInsights,
  type AgentInsightsStream,
} from '@/lib/ipc/agent-insights-stream'

export type Part = 'report' | 'calendar'

export interface ReportRequest {
  range: InsightsRange
  harnesses: AgentToolId[]
  projectId: string | null
  /** `null`: this year in the shown zone. */
  year: number | null
  /** The zone Insights show; the backend reads in it, so a change reads again. */
  zone: string
  /** The cost switches, which change what the report holds. */
  cost: string
}

/** A section and the request it answers. */
interface Held<T> {
  value: T
  key: string
}

type Started = Extract<InsightsFrame, { type: 'started' }>['payload']

interface Sections {
  started: Started | null
  work: Held<InsightsWork> | null
  overview: Held<InsightsOverview> | null
  series: Held<InsightsSeries> | null
  breakdowns: Held<InsightsBreakdowns> | null
  calendar: Held<InsightsCalendar> | null
  /** How the latest request for each part ended. */
  ended: Record<Part, { key: string; code: string | null } | null>
}

type Action =
  | { type: 'started'; value: Started }
  | {
      type: 'section'
      name: 'work' | 'overview' | 'series' | 'breakdowns' | 'calendar'
      value: unknown
      key: string
    }
  | { type: 'ended'; part: Part; key: string; code: string | null }

function reduce(state: Sections, action: Action): Sections {
  if (action.type === 'started') return { ...state, started: action.value }
  if (action.type === 'section')
    return { ...state, [action.name]: { value: action.value, key: action.key } }
  return {
    ...state,
    ended: {
      ...state.ended,
      [action.part]: { key: action.key, code: action.code },
    },
  }
}

const empty: Sections = {
  started: null,
  work: null,
  overview: null,
  series: null,
  breakdowns: null,
  calendar: null,
  ended: { report: null, calendar: null },
}

/** The backend's code when a backup published while the report was read. */
const STALE_VIEW = 'stale_cursor'
const MAX_RESTARTS = 3

/** One request to the backend, which may answer for one part or both. */
interface Live {
  keys: Partial<Record<Part, string>>
  handle: AgentInsightsStream | null
  closed: boolean
}

export interface PartState {
  /** Sections for exactly the current choices have all arrived. */
  done: boolean
  /** The backend's code when the latest request failed. */
  failed: string | null
}

export function useInsightsReport(request: ReportRequest, enabled: boolean) {
  const [state, dispatch] = useReducer(reduce, empty)
  const [ticks, setTicks] = useState({ report: 0, calendar: 0 })
  const harnesses = [...request.harnesses].sort().join(',')
  const scope = `${harnesses}|${request.projectId ?? ''}|${request.zone}`
  const reportKey = `${request.range}|${scope}|${request.cost}|${ticks.report}`
  const calendarKey = `${request.year ?? 'now'}|${scope}|${ticks.calendar}`

  const streams = useRef<Record<Part, Live | null>>({
    report: null,
    calendar: null,
  })
  const asked = useRef<Record<Part, string | null>>({
    report: null,
    calendar: null,
  })
  /** Reads started again in a row because the archive moved on. */
  const restarts = useRef(0)
  /** Asks for `keys`' parts with the choices as they are now. */
  const ask = useEffectEvent((keys: Partial<Record<Part, string>>) => {
    const parts = Object.keys(keys) as Part[]
    const live: Live = { keys, handle: null, closed: false }
    const before = { ...streams.current }
    for (const part of parts) {
      streams.current[part] = live
      asked.current[part] = keys[part]!
    }
    // A request no part waits for any more is stopped. One that still
    // carries the other part keeps running for it.
    for (const old of new Set(Object.values(before))) {
      if (!old || old.closed) continue
      if (streams.current.report !== old && streams.current.calendar !== old) {
        old.closed = true
        old.handle?.close()
      }
    }

    const current = (part: Part) => streams.current[part] === live
    const onFrame = (frame: InsightsFrame) => {
      switch (frame.type) {
        case 'started':
          if (current('report') || current('calendar'))
            dispatch({ type: 'started', value: frame.payload })
          return
        case 'work':
        case 'overview':
        case 'series':
        case 'breakdowns':
          if (current('report'))
            dispatch({
              type: 'section',
              name: frame.type,
              value: frame.payload,
              key: live.keys.report!,
            })
          return
        case 'calendar':
          if (current('calendar'))
            dispatch({
              type: 'section',
              name: 'calendar',
              value: frame.payload,
              key: live.keys.calendar!,
            })
          return
        case 'finished':
        case 'cancelled':
        case 'error': {
          const code = frame.type === 'finished' ? null : frame.payload.code
          // A backup published while this was read: read the new view.
          const again = code === STALE_VIEW && restarts.current < MAX_RESTARTS
          if (current('report') || current('calendar'))
            restarts.current = again ? restarts.current + 1 : 0
          for (const part of ['report', 'calendar'] as const) {
            if (!current(part)) continue
            streams.current[part] = null
            if (again) setTicks((now) => ({ ...now, [part]: now[part] + 1 }))
            else dispatch({ type: 'ended', part, key: live.keys[part]!, code })
          }
          live.closed = true
          return
        }
        default:
          return
      }
    }

    const { range, projectId, year } = request
    const query: InsightsQuery = {
      range,
      part: parts.length === 2 ? 'both' : parts[0],
      harnesses: request.harnesses,
      ...(projectId ? { projectId } : {}),
      ...(year !== null ? { year } : {}),
    }
    openAgentInsights(query, onFrame).then(
      (handle) => {
        live.handle = handle
        if (live.closed) handle.close()
      },
      (error: unknown) => {
        const code =
          error &&
          typeof error === 'object' &&
          'code' in error &&
          typeof error.code === 'string'
            ? error.code
            : describeError(error)
        onFrame({
          streamId: '',
          sequence: '',
          type: 'error',
          payload: { code },
        })
      },
    )
  })

  useEffect(() => {
    if (!enabled) return
    const keys: Partial<Record<Part, string>> = {}
    if (asked.current.report !== reportKey) keys.report = reportKey
    if (asked.current.calendar !== calendarKey) keys.calendar = calendarKey
    if (Object.keys(keys).length > 0) ask(keys)
  }, [enabled, reportKey, calendarKey])

  // Leaving the screen stops whatever is still running; coming back
  // (StrictMode does at once) asks again.
  useEffect(
    () => () => {
      for (const live of Object.values(streams.current)) {
        if (!live || live.closed) continue
        live.closed = true
        live.handle?.close()
      }
      streams.current = { report: null, calendar: null }
      asked.current = { report: null, calendar: null }
    },
    [],
  )

  // A backup, a mark (star, trash) or a project change moved conversations.
  useEffect(() => {
    let stop: (() => void) | null = null
    let gone = false
    void subscribeToAgentConversations(() =>
      setTicks((now) => ({
        report: now.report + 1,
        calendar: now.calendar + 1,
      })),
    ).then((unlisten) => {
      if (gone) unlisten()
      else stop = unlisten
    })
    return () => {
      gone = true
      stop?.()
    }
  }, [])

  const retry = useCallback(
    (part: Part) => setTicks((now) => ({ ...now, [part]: now[part] + 1 })),
    [],
  )

  const partState = (part: Part, key: string): PartState => {
    const ended = state.ended[part]
    const mine = ended?.key === key
    return {
      done: mine && ended.code === null,
      failed: mine ? ended.code : null,
    }
  }
  const fresh = <T>(held: Held<T> | null, key: string) =>
    held === null ? null : { value: held.value, stale: held.key !== key }

  return {
    started: state.started,
    report: {
      ...partState('report', reportKey),
      overview: fresh(state.overview, reportKey),
      series: fresh(state.series, reportKey),
      breakdowns: fresh(state.breakdowns, reportKey),
      /** Early and unfiltered: only until `series` for these choices. */
      work: state.work?.key === reportKey ? state.work.value : null,
    },
    calendar: {
      ...partState('calendar', calendarKey),
      data: fresh(state.calendar, calendarKey),
    },
    retry,
  }
}

export type InsightsReport = ReturnType<typeof useInsightsReport>
/** A section on screen, and whether it answers older choices. */
export type Shown<T> = { value: T; stale: boolean } | null
