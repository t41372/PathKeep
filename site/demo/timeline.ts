/**
 * Every visit in the demo archive, newest first, the way History lists them,
 * kept compact: each page (address, title, site) and each profile written
 * once, and every visit as a row of numbers pointing at them. The demo works
 * out any History list from it (a date range, a browser, a site, a page
 * further down); `record.ts` checks that unpacking gives back exactly what
 * the backend sent.
 */
import type { HistoryEntry } from '../../src/lib/types/archive.ts'

export interface PackedTimeline {
  /** Address, title, site. */
  pages: [string, string | null, string][]
  profiles: string[]
  /** The app a visit came through, where the browser says (Firefox does). */
  apps: (string | null)[]
  /** id, profile, page, visit time, duration, transition, source visit id, app. */
  visits: [
    number,
    number,
    number,
    number,
    number | null,
    number | null,
    number,
    number,
  ][]
}

/** The backend's `visitedAt`: RFC 3339 in UTC, milliseconds only when there are any. */
const visitedAt = (time: number) =>
  new Date(time).toISOString().replace('.000Z', 'Z').replace('Z', '+00:00')

export function packTimeline(entries: HistoryEntry[]): PackedTimeline {
  const pages = new Map<string, number>()
  const profiles = new Map<string, number>()
  const apps = new Map<string | null, number>()
  const packed: PackedTimeline = {
    pages: [],
    profiles: [],
    apps: [],
    visits: [],
  }
  for (const entry of entries) {
    const pageKey = JSON.stringify([
      entry.url,
      entry.title ?? null,
      entry.domain,
    ])
    if (!pages.has(pageKey)) {
      pages.set(pageKey, packed.pages.length)
      packed.pages.push([entry.url, entry.title ?? null, entry.domain])
    }
    if (!profiles.has(entry.profileId)) {
      profiles.set(entry.profileId, packed.profiles.length)
      packed.profiles.push(entry.profileId)
    }
    const app = entry.appId ?? null
    if (!apps.has(app)) {
      apps.set(app, packed.apps.length)
      packed.apps.push(app)
    }
    packed.visits.push([
      entry.id,
      profiles.get(entry.profileId)!,
      pages.get(pageKey)!,
      entry.visitTime,
      entry.durationMs ?? null,
      entry.transition ?? null,
      entry.sourceVisitId,
      apps.get(app)!,
    ])
  }
  return packed
}

export function unpackTimeline(packed: PackedTimeline): HistoryEntry[] {
  return packed.visits.map(
    ([id, profile, page, time, durationMs, transition, sourceVisitId, app]) => {
      const [url, title, domain] = packed.pages[page]
      return {
        id,
        profileId: packed.profiles[profile],
        url,
        title,
        domain,
        favicon: null,
        visitedAt: visitedAt(time),
        visitTime: time,
        durationMs,
        transition,
        sourceVisitId,
        appId: packed.apps[app],
        enrichmentExcerpt: null,
      }
    },
  )
}
