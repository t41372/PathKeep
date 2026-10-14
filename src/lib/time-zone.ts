/**
 * Calendar days in a named time zone (an IANA name such as `Asia/Taipei`).
 * Insights put visits on days in a zone of their own, which can differ from
 * the computer's (`docs/architecture/insights-time-zone.md`), so their days
 * cannot come from `Date`'s local getters.
 *
 * Responsible for: the day an instant falls on in a zone, today there, day
 * arithmetic on `YYYY-MM-DD` keys, the computer's zone, a zone's name for
 * people. Not responsible for: which zone Insights use (`insights/zone.ts`),
 * formatting dates and times (`useFormat`).
 *
 * Day keys are plain calendar dates: arithmetic on them goes through UTC
 * midnight, where no day is 23 or 25 hours long.
 */

const DAY_MS = 86_400_000
const keyPattern = /^\d{4}-\d{2}-\d{2}$/
const keyFormats = new Map<string, Intl.DateTimeFormat>()

function keyFormat(zone: string) {
  let format = keyFormats.get(zone)
  if (!format) {
    format = new Intl.DateTimeFormat('en-US', {
      timeZone: zone,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    })
    keyFormats.set(zone, format)
  }
  return format
}

/** The `YYYY-MM-DD` day the instant `ms` falls on in `zone`. */
export function dateKeyIn(ms: number, zone: string): string {
  const parts = keyFormat(zone).formatToParts(ms)
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((item) => item.type === type)?.value ?? ''
  return `${part('year')}-${part('month')}-${part('day')}`
}

/** Today in `zone`. */
export function todayIn(zone: string, now = Date.now()): string {
  return dateKeyIn(now, zone)
}

function utcMidnight(key: string) {
  const [year, month, day] = key.split('-').map(Number)
  return Date.UTC(year, month - 1, day)
}

function keyOfUtc(ms: number) {
  return new Date(ms).toISOString().slice(0, 10)
}

/** A real calendar day written `YYYY-MM-DD`. */
export function isDateKey(value: string | null | undefined): value is string {
  return (
    !!value && keyPattern.test(value) && keyOfUtc(utcMidnight(value)) === value
  )
}

/** The day `days` after `key` (before, when negative). */
export function addDays(key: string, days: number): string {
  return keyOfUtc(utcMidnight(key) + days * DAY_MS)
}

/** Whole days from `start` to `end`. */
export function daysBetween(start: string, end: string): number {
  return Math.round((utcMidnight(end) - utcMidnight(start)) / DAY_MS)
}

/** The day `key` at UTC midnight, for formatting as a calendar date in `UTC`. */
export function dayAsUtcDate(key: string): Date {
  return new Date(utcMidnight(key))
}

/** The computer's zone, as this window sees it (the zone History shows days in). */
export function computerTimeZone(): string {
  return new Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
}

/** The same zone under another of its names (`Asia/Calcutta` and `Asia/Kolkata`) counts as the same. */
export function sameZone(a: string, b: string): boolean {
  if (a === b) return true
  try {
    const canonical = (zone: string) =>
      new Intl.DateTimeFormat('en-US', { timeZone: zone }).resolvedOptions()
        .timeZone
    return canonical(a) === canonical(b)
  } catch {
    return false
  }
}

/** Every zone this window knows, plus `extra` (a zone saved elsewhere may be missing from the list). */
export function knownTimeZones(extra: string[] = []): string[] {
  const listed =
    typeof Intl.supportedValuesOf === 'function'
      ? Intl.supportedValuesOf('timeZone')
      : []
  return [...new Set([...listed, 'UTC', ...extra])]
}

function zonePart(
  zone: string,
  locale: string,
  style: Intl.DateTimeFormatOptions['timeZoneName'],
) {
  try {
    return (
      new Intl.DateTimeFormat(locale, { timeZone: zone, timeZoneName: style })
        .formatToParts(Date.now())
        .find((part) => part.type === 'timeZoneName')?.value ?? ''
    )
  } catch {
    return ''
  }
}

/** A zone as people know it: its place, its name in `locale` and its offset now. */
export interface ZoneLabel {
  /** The place in the IANA name: `Los Angeles` for `America/Los_Angeles`. */
  place: string
  /** `Pacific Time`, `太平洋時間`; empty when the language has no name for it. */
  name: string
  /** `GMT-7` */
  offset: string
}

const labels = new Map<string, ZoneLabel>()

/** Its label, worked out once per zone and language (each takes two `Intl` formatters). */
export function zoneLabel(zone: string, locale: string): ZoneLabel {
  const key = `${locale}|${zone}`
  let label = labels.get(key)
  if (!label) {
    const place = (zone.split('/').at(-1) ?? zone).replaceAll('_', ' ')
    const name = zonePart(zone, locale, 'longGeneric')
    label = {
      place,
      // Zones without a name of their own come back as their offset ("GMT+00:00").
      name: /^GMT[+\-−]?/.test(name) ? '' : name,
      offset: zonePart(zone, locale, 'shortOffset'),
    }
    labels.set(key, label)
  }
  return label
}

/** Whether `zoneLabel` has this one already. */
export function hasZoneLabel(zone: string, locale: string): boolean {
  return labels.has(`${locale}|${zone}`)
}
