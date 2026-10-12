/**
 * What kind of site each fixture visit went to, decided by hand, so specs
 * count "Kinds of sites" without asking PathKeep's own lists.
 */
import type { FixtureVisit } from './fixture'

/**
 * The kind of site a fixture visit went to, decided by hand from what each
 * site is: a search on Google is a search; GitHub is for building software;
 * docs.rs, SQLite's and Tauri's sites are documentation, and so is any page
 * under /docs, /doc, /reference, /api, /guide or /guides on any site (the
 * `docs-path` rule); YouTube is video; Reddit and Hacker News are forums; B&H
 * and Amazon are shops. The rest (personal blogs, Strava, a JLPT course,
 * camera reviews, running advice) are on no list, whatever their page titles
 * say.
 */
export function kindOf(visit: FixtureVisit) {
  if (visit.term) return 'Search'
  const url = new URL(visit.url)
  switch (url.hostname) {
    case 'github.com':
      return 'Building software'
    case 'docs.rs':
    case 'www.sqlite.org':
    case 'v2.tauri.app':
      return 'Documentation'
    case 'www.youtube.com':
      return 'Video'
    case 'www.reddit.com':
    case 'news.ycombinator.com':
      return 'Forums and Q&A'
    case 'www.bhphotovideo.com':
    case 'www.amazon.com':
    case 'www.amazon.de':
      return 'Shopping'
  }
  return DOCS_PATHS.some((prefix) => url.pathname.startsWith(prefix))
    ? 'Documentation'
    : 'Not classified'
}

const DOCS_PATHS = ['/docs', '/doc', '/reference', '/api', '/guide', '/guides']
