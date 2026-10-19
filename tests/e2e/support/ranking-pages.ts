/**
 * Browsing `search-ranking.spec` adds to Chrome's history: pages whose
 * search order is known from the ranking rule alone (recall.md: score, then
 * the newest matching visit, then page id). Times are relative to when the
 * spec runs; `wipe.spec` counts the visits.
 */
const HOUR_MS = 3_600_000
const DAY_MS = 24 * HOUR_MS

export interface RankingPage {
  url: string
  title: string
  /** How long ago each visit was, newest last. */
  ago: number[]
}

/** "lighthouse" is in the first page's title and only in the second's address. */
export const IN_TITLE: RankingPage = {
  url: 'https://ranking.e2e.example/notes/1',
  title: 'Lighthouse keeper journal',
  ago: [3 * DAY_MS],
}
/** Visited more often and more recently, so only the title can put the other first. */
export const IN_ADDRESS: RankingPage = {
  url: 'https://ranking.e2e.example/lighthouse',
  title: 'Weekly plan',
  ago: [4 * HOUR_MS, 3 * HOUR_MS, 2 * HOUR_MS, HOUR_MS],
}
/** The same title on two pages: the one visited last comes first. */
export const OLDER_TWIN: RankingPage = {
  url: 'https://ranking.e2e.example/ferry/a',
  title: 'Ferry timetable',
  ago: [2 * DAY_MS],
}
export const NEWER_TWIN: RankingPage = {
  url: 'https://ranking.e2e.example/ferry/b',
  title: 'Ferry timetable',
  ago: [DAY_MS],
}

export const RANKING_PAGES = [IN_TITLE, IN_ADDRESS, OLDER_TWIN, NEWER_TWIN]

export function rankingVisitCount() {
  return RANKING_PAGES.reduce((sum, page) => sum + page.ago.length, 0)
}
