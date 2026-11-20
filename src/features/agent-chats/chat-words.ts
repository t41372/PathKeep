/**
 * How the list puts a conversation into words, the same wherever one is
 * listed: its title (or that it has none), its time on a row and a day
 * group's heading, and the number a metric sort puts on the row.
 *
 * Times are in Insights' zone, the zone the backend's day groups and
 * ranges use, so a row never sits under a day it does not show.
 *
 * Responsibilities: choosing the words. Not responsible for: the copy
 * (`agentChats` messages) or where it is drawn.
 */
import type {
  ChatSort,
  ChatSummary,
  TitleSource,
} from '@/lib/backend-client/agent-chats'
import type { Formatters, Translator } from '@/lib/i18n'
import { addDays, dateKeyIn, daysBetween, todayIn } from '@/lib/time-zone'

export function chatTitle(t: Translator, summary: Pick<ChatSummary, 'title'>) {
  return summary.title?.trim() || t('agentChats.row.untitled')
}

/**
 * A title taken from the first prompt reads lighter than one the tool or
 * the person gave (`agent-history.md` §3.3).
 */
export function fromFirstPrompt(source: TitleSource) {
  return source === 'firstPrompt'
}

/** The time a row shows: its start when sorted by start, else its last activity. */
export function rowInstant(summary: ChatSummary, sort: ChatSort) {
  return sort === 'started' ? summary.startedAt : summary.lastActivityAt
}

/**
 * A row's time: only the clock under a day heading; otherwise "Today
 * 14:32", "Yesterday 14:32", the weekday this week, the date this year,
 * the full date before.
 */
export function rowWhen(
  t: Translator,
  f: Formatters,
  zone: string,
  at: number | null,
  clockOnly: boolean,
  now = Date.now(),
) {
  if (at === null) return ''
  if (clockOnly) return f.time(at)
  const day = dateKeyIn(at, zone)
  const today = todayIn(zone, now)
  const ago = daysBetween(day, today)
  if (ago === 0) return t('agentChats.when.today', { time: f.time(at) })
  if (ago === 1) return t('agentChats.when.yesterday', { time: f.time(at) })
  if (ago > 1 && ago < 7)
    return t('agentChats.when.weekday', {
      weekday: f.date(at, { weekday: 'short' }),
      time: f.time(at),
    })
  if (day.slice(0, 4) === today.slice(0, 4))
    return f.date(at, { month: 'short', day: 'numeric' })
  return f.date(at, { dateStyle: 'medium' })
}

/** A day group's heading: "Today · Fri, Nov 13", "Nov 10 Tue", with the year when not this year. */
export function dayHeading(
  t: Translator,
  f: Formatters,
  zone: string,
  day: string | null,
  now = Date.now(),
) {
  if (day === null) return t('agentChats.groups.undated')
  const today = todayIn(zone, now)
  const date = f.day(day, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: day.slice(0, 4) === today.slice(0, 4) ? undefined : 'numeric',
  })
  if (day === today) return t('agentChats.groups.today', { date })
  if (day === addDays(today, -1))
    return t('agentChats.groups.yesterday', { date })
  return date
}

/** Hours, or minutes under an hour: a metric sort's number for agent time. */
export function hoursText(t: Translator, f: Formatters, ms: number) {
  const minutes = ms / 60_000
  if (minutes < 60)
    return t('agentChats.row.minutes', { count: f.number(Math.round(minutes)) })
  const hours = minutes / 60
  return t('agentChats.row.hours', {
    count: f.number(
      hours < 10 ? Math.round(hours * 10) / 10 : Math.round(hours),
    ),
  })
}

/** The number a metric sort shows on each row; nothing recorded is said, never 0. */
export function metricText(
  t: Translator,
  f: Formatters,
  sort: ChatSort,
  summary: ChatSummary,
) {
  if (sort === 'agentTime')
    return summary.agentMs === null
      ? t('agentChats.row.noRecord')
      : hoursText(t, f, summary.agentMs)
  if (sort === 'tokens')
    return summary.tokensTotal === null
      ? t('agentChats.row.noRecord')
      : t('agentChats.row.tokens', { count: f.compact(summary.tokensTotal) })
  if (sort === 'turns')
    return t('agentChats.row.turns', { count: summary.turns })
  return null
}
