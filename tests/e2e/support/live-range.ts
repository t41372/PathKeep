/**
 * The visits `backup.spec` adds while Insights stay open on the last seven
 * days: a few more to the range's top site and many to a site the archive
 * has never seen. `wipe.spec` counts them in what the archive holds.
 */
import type { FixtureVisit } from './fixture'

/** A site of its own: under e2e.example it would share a site with other specs' pages. */
export const LIVE_RANGE_SITE = 'live-range.example'
export const LIVE_RANGE_TOP_SITE_VISITS = 6
export const LIVE_RANGE_NEW_SITE_VISITS = 30

/**
 * Visits a minute apart, ending a minute ago: a few to `topSitePage` (a page
 * of the range's top site, under its own title) for Chrome, then many to the
 * new site for Firefox. Two profiles, so the backup queues an Insights build
 * for each.
 */
export function liveRangeVisits(topSitePage: { url: string; title: string }) {
  const pages = [
    ...Array.from({ length: LIVE_RANGE_TOP_SITE_VISITS }, () => topSitePage),
    ...Array.from({ length: LIVE_RANGE_NEW_SITE_VISITS }, (_, index) => ({
      url: `https://${LIVE_RANGE_SITE}/day/${index % 5}`,
      title: `Live range day ${index % 5}`,
    })),
  ]
  const now = Date.now()
  const visits: FixtureVisit[] = pages.map(({ url, title }, index) => ({
    url,
    title,
    at: now - (pages.length - index) * 60_000,
    typed: true,
    duration: 30_000,
  }))
  return {
    chrome: visits.slice(0, LIVE_RANGE_TOP_SITE_VISITS),
    firefox: visits.slice(LIVE_RANGE_TOP_SITE_VISITS),
  }
}

export function liveRangeVisitCount() {
  return LIVE_RANGE_TOP_SITE_VISITS + LIVE_RANGE_NEW_SITE_VISITS
}
