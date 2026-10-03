/** Everything the History detail panel shows about one URL. */
import { call } from './shared'

export interface UrlDetail {
  url: string
  title: string | null
  domain: string
  totalVisits: number
  firstVisitAt: string | null
  lastVisitAt: string | null
  /** Browser names (e.g. "Arc") that recorded this URL, most visits first. */
  browsers: string[]
  /** Exactly 12 entries, oldest first, ending with the current week (Monday start, local time). */
  weeklyVisits: { weekStart: string; visits: number }[]
}

export const urlDetailClient = {
  get: (url: string) => call<UrlDetail>('get_url_detail', { url }),
}
