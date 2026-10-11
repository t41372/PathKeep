export interface SyntheticVisit {
  url: string
  title: string
  /** Milliseconds since the epoch. */
  at: number
  typed: boolean
  /** Milliseconds. */
  duration: number
  /** Set on search result pages: the query that was typed. */
  term?: string
  /** Set when the visit must be in this profile; others are split by a hash. */
  profile?: SyntheticProfileId
}

export type SyntheticProfileId =
  | 'chrome:Default'
  | 'chrome:Profile 1'
  | 'firefox:k3x9.default-release'

export interface SyntheticBrowsers {
  chromeUserDataRoot: string
  firefoxProfilesRoot: string
  visitsByProfile: Record<SyntheticProfileId, SyntheticVisit[]>
  files: Record<SyntheticProfileId, string>
  counts: { total: number; personal: number; work: number; firefox: number }
}

export function generateVisits(options?: {
  now?: number
  days?: number
  olderDays?: number
  perDay?: number
  seed?: number
}): SyntheticVisit[]

export function writeSyntheticBrowsers(
  root: string,
  options?: {
    now?: number
    days?: number
    olderDays?: number
    perDay?: number
    seed?: number
  },
): SyntheticBrowsers

export function appendChromeVisits(file: string, visits: SyntheticVisit[]): void

export function appendFirefoxVisits(
  file: string,
  visits: SyntheticVisit[],
): void

/** Creates a new Firefox places.sqlite holding `visits`. */
export function writeFirefoxPlaces(file: string, visits: SyntheticVisit[]): void

/** Creates a new Safari History.db holding `visits`; a visit's title may be null, as in Safari. */
export function writeSafariHistory(
  file: string,
  visits: readonly { url: string; title: string | null; at: number }[],
): void

/** The words of the fixture's Chinese searches, as Insights splits them. */
export const FIXTURE_CJK_WORDS: Record<string, string[]>

/** A second site on github.io, visited three times two days before `now`. */
export const FIXTURE_PAGES_SITE: {
  site: string
  pages: [url: string, title: string][]
}

/** A search that compared one product on two shops, and the pages it opened in order. */
export const FIXTURE_COMPARE: {
  term: string
  /** URL and title of each page. */
  pages: [string, string][]
  /** Which page each visit after the search opened. */
  order: number[]
}

/** Host → base64 PNG of the icons Chrome stored in the personal profile's `Favicons`. */
export const FIXTURE_FAVICON_HOSTS: Record<string, string>

/** Pages whose stored icon is not their site's (newer than the site's). */
export const FIXTURE_PAGE_ICONS: Record<string, string>

/**
 * For each visit (in time order), the index of the visit it was opened from,
 * or -1 (written as the browsers' `from_visit`).
 */
export function openedFrom(
  visits: readonly Pick<SyntheticVisit, 'at' | 'typed' | 'term'>[],
): number[]
