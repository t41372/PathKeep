/**
 * How Insights splits searches into words, follows a search through the pages
 * it opened, and groups searches into query families, done by hand for the
 * fixture, so the specs count search words, trails and families by the
 * documented rules (`deterministic-intelligence.md` §5.4).
 *
 * The fixture's Chinese searches are spelled out (`FIXTURE_CJK_WORDS`) rather
 * than run through a segmenter: Node's `Intl.Segmenter` uses ICU4C data,
 * which need not split them as the backend's ICU4X does. vault-core's
 * tokenizer test pins the backend to the same words.
 */
import {
  FIXTURE_CJK_WORDS,
  openedFrom,
} from '../../../scripts/fixtures/synthetic-browsers.mjs'
import type { FixtureVisit } from './fixture'

/** English function words and URL parts, as the backend leaves them out (`is_stop_word`). */
const fillerWords = new Set(
  `a about above after again against all am an and any are as at be because been before
  being below between both but by can com could did do does doing down during each edu few
  for from further had has have having he her here hers herself him himself his how html
  http https i if in into is it its itself just me more most my myself net no nor not of
  off on once only or org other our ours ourselves out over own same she should so some
  such than that the their theirs them themselves then there these they this those through
  to too under until up versus very vs was we were what when where which while who whom
  why will with would www you your yours yourself yourselves`.split(/\s+/),
)

const cjk =
  /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u

/** A text's tokens: lower case, split at anything not a letter or digit. */
function tokens(text: string) {
  return text
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
}

/**
 * The words a search is shown and counted under ("What you search for"):
 * Latin words without filler words, single letters or bare numbers, and the
 * fixture's Chinese searches as their words.
 */
export function searchWords(term: string) {
  return new Set(
    tokens(term)
      .flatMap((token) => FIXTURE_CJK_WORDS[token] ?? [token])
      .filter(
        (word) =>
          !fillerWords.has(word) &&
          (cjk.test(word) || (word.length > 1 && !/^\d+$/.test(word))),
      ),
  )
}

/**
 * The set two searches, or a page title and a search, are compared by: Latin
 * words without filler words; a Chinese phrase as its overlapping pairs of
 * characters (the fixture's hold no function words to take out first).
 */
export function compareTokens(text: string) {
  const set = new Set<string>()
  for (const token of tokens(text)) {
    if (!cjk.test(token)) {
      if (!fillerWords.has(token)) set.add(token)
      continue
    }
    const chars = [...token]
    if (chars.length === 1) set.add(token)
    for (let index = 1; index < chars.length; index += 1)
      set.add(chars[index - 1] + chars[index])
  }
  return set
}

/** The registrable domain of a fixture URL: every fixture host sits under a one-label suffix. */
function site(url: string) {
  return new URL(url).hostname.split('.').slice(-2).join('.')
}

/** How many tokens two searches have in common, and how many either has. */
function overlap(search: Set<string>, other: Set<string>) {
  const shared = [...search].filter((token) => other.has(token)).length
  return { shared, union: new Set([...search, ...other]).size }
}

/** Whether a search rewords the trail's latest: half their tokens in common, or one holds the other. */
function rewords(search: Set<string>, latest: Set<string>) {
  const { shared, union } = overlap(search, latest)
  return shared * 2 >= union || shared === search.size || shared === latest.size
}

/**
 * How much a search is like a family's first search: the share of their
 * tokens they have in common, or `null` when it may not join (under half, or
 * fewer than two in common, unless they have the same tokens).
 */
function likeness(search: Set<string>, first: Set<string>) {
  const { shared, union } = overlap(search, first)
  const same = shared === search.size && shared === first.size
  return same || (shared >= 2 && shared * 2 >= union) ? shared / union : null
}

export interface SearchTrail {
  /** The search as first typed (`initial_query`). */
  query: string
  rewordings: number
  /** Pages it opened, its rewordings not counted (`max_depth`). */
  pages: number
  latest: Set<string>
  landing?: string
  /** The search and each rewording of it, in order. */
  searches: FixtureVisit[]
}

/**
 * One profile's search trails: a search continues the current trail when it
 * is in the same session (30 minutes), the trail's last visit is under 15
 * minutes old, and it rewords the trail's latest wording (`rewords`). A page
 * belongs to the trail when it was opened from one of the trail's visits, is
 * on a site the trail went to, or its title shares a word with the trail's
 * searches; three pages in a row that do not end it.
 */
export function searchTrails(visits: FixtureVisit[]): SearchTrail[] {
  const SESSION_GAP = 30 * 60_000
  const TRAIL_GAP = 15 * 60_000
  const trails: SearchTrail[] = []
  let lastVisit = -Infinity
  let trail:
    | (SearchTrail & {
        words: Set<string>
        members: Set<number>
        sites: Set<string>
        unrelated: number
        lastAt: number
        landsNext: boolean
      })
    | null = null
  const ordered = [...visits].sort((a, b) => a.at - b.at)
  const from = openedFrom(ordered)
  for (const [index, visit] of ordered.entries()) {
    const sameSession = visit.at - lastVisit <= SESSION_GAP
    lastVisit = visit.at
    const open = trail && sameSession && visit.at - trail.lastAt <= TRAIL_GAP
    if (visit.term) {
      const words = compareTokens(visit.term)
      if (open && rewords(words, trail!.latest)) {
        if ([...words].join(' ') !== [...trail!.latest].join(' '))
          trail!.rewordings += 1
        trail!.latest = words
        for (const word of words) trail!.words.add(word)
        trail!.members.add(index)
        trail!.searches.push(visit)
        trail!.unrelated = 0
        trail!.lastAt = visit.at
        // The page opened after the last wording becomes the landing.
        trail!.landsNext = true
      } else {
        trail = {
          query: visit.term,
          rewordings: 0,
          pages: 0,
          latest: words,
          searches: [visit],
          words: new Set(words),
          members: new Set([index]),
          sites: new Set(),
          unrelated: 0,
          lastAt: visit.at,
          landsNext: false,
        }
        trails.push(trail)
      }
    } else if (open) {
      const relates =
        (from[index] >= 0 && trail!.members.has(from[index])) ||
        trail!.sites.has(site(visit.url)) ||
        [...compareTokens(visit.title)].some((word) => trail!.words.has(word))
      if (relates) {
        trail!.pages += 1
        trail!.members.add(index)
        trail!.sites.add(site(visit.url))
        trail!.unrelated = 0
        trail!.lastAt = visit.at
        if (trail!.landing === undefined || trail!.landsNext) {
          trail!.landing = site(visit.url)
          trail!.landsNext = false
        }
      } else if (++trail!.unrelated >= 3) {
        trail = null
      }
    } else {
      trail = null
    }
  }
  return trails
}

export interface SearchFamily {
  /** The search that started it, as typed. */
  anchor: string
  /** Every wording in it, lower case. */
  queries: Set<string>
  /** How many searches it holds. */
  members: number
  /** The local days with a search in it, `YYYY-M-D`. */
  days: Set<string>
  /** When its newest search was made. */
  latest: number
}

/**
 * One profile's query families by the documented rule: each search, in time
 * order, joins the family that already holds its wording; else, when it
 * continues the trail of the search just before it, that search's family;
 * else the family whose first search it is most like (the earliest made on a
 * tie); otherwise it starts one. (Every fixture search is on Google.) The
 * specs count with this; `home.spec` checks separately that the families come
 * out as the fixture means them, whatever the rule.
 */
export function searchFamilies(visits: FixtureVisit[]) {
  const trailOf = new Map<FixtureVisit, number>()
  for (const [index, trail] of searchTrails(visits).entries())
    for (const search of trail.searches) trailOf.set(search, index)
  const families: (SearchFamily & { first: Set<string> })[] = []
  let last: { trail: number; family: (typeof families)[number] } | null = null
  for (const visit of [...visits].sort((a, b) => a.at - b.at)) {
    if (!visit.term) continue
    const search = compareTokens(visit.term)
    if (search.size === 0) {
      last = null
      continue
    }
    const trail = trailOf.get(visit)
    const day = new Date(visit.at)
    const key = `${day.getFullYear()}-${day.getMonth()}-${day.getDate()}`
    let family = families.find((candidate) =>
      candidate.queries.has(visit.term!.toLowerCase()),
    )
    if (!family && last && last.trail === trail) family = last.family
    if (!family) {
      let best = -1
      for (const candidate of families) {
        const score = likeness(search, candidate.first)
        if (score !== null && score > best) {
          best = score
          family = candidate
        }
      }
    }
    if (family) {
      family.members += 1
      family.queries.add(visit.term.toLowerCase())
      family.days.add(key)
      family.latest = Math.max(family.latest, visit.at)
    } else {
      family = {
        anchor: visit.term,
        first: search,
        queries: new Set([visit.term.toLowerCase()]),
        members: 1,
        days: new Set([key]),
        latest: visit.at,
      }
      families.push(family)
    }
    last = trail === undefined ? null : { trail, family }
  }
  return families
}
