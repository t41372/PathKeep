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
  perDay?: number
  seed?: number
}): SyntheticVisit[]

export function writeSyntheticBrowsers(
  root: string,
  options?: { now?: number; days?: number; perDay?: number; seed?: number },
): SyntheticBrowsers

export function appendChromeVisits(file: string, visits: SyntheticVisit[]): void

export function appendFirefoxVisits(
  file: string,
  visits: SyntheticVisit[],
): void
