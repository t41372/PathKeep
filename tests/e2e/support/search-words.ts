/**
 * How Insights splits searches into words, done by hand for the fixture, so
 * the specs count search words, trails and query families the way the
 * backend does (`intelligence_shared.rs`, `intelligence_structural_build.rs`).
 *
 * The fixture's Chinese searches are spelled out (`FIXTURE_CJK_WORDS`) rather
 * than run through a segmenter: Node's `Intl.Segmenter` uses ICU4C data,
 * which need not split them as the backend's ICU4X does. vault-core's
 * tokenizer test pins the backend to the same words.
 */
import { FIXTURE_CJK_WORDS } from '../../../scripts/fixtures/synthetic-browsers.mjs'
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

/**
 * How much a search is like one wording of a family: the share of their
 * tokens they have in common, or `null` when that is under half and neither
 * holds the other (`deterministic-intelligence.md` §5.5).
 */
function likeness(search: Set<string>, wording: Set<string>) {
  const shared = [...search].filter((token) => wording.has(token)).length
  const union = new Set([...search, ...wording]).size
  const like =
    shared * 2 >= union || shared === search.size || shared === wording.size
  return like ? shared / union : null
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
 * order, joins the family that already holds its wording, else the family
 * with the wording most like it (the earliest made on a tie); otherwise it
 * starts one. (Every fixture search is on Google.) The specs count with this;
 * `home.spec` checks separately that the families come out as the fixture
 * means them, whatever the rule.
 */
export function searchFamilies(visits: FixtureVisit[]) {
  const families: (SearchFamily & { wordings: Set<string>[] })[] = []
  for (const visit of [...visits].sort((a, b) => a.at - b.at)) {
    if (!visit.term) continue
    const search = compareTokens(visit.term)
    if (search.size === 0) continue
    const day = new Date(visit.at)
    const key = `${day.getFullYear()}-${day.getMonth()}-${day.getDate()}`
    let family = families.find((candidate) =>
      candidate.queries.has(visit.term!.toLowerCase()),
    )
    if (!family) {
      let best = -1
      for (const candidate of families)
        for (const wording of candidate.wordings) {
          const score = likeness(search, wording)
          if (score !== null && score > best) {
            best = score
            family = candidate
          }
        }
    }
    if (family) {
      family.members += 1
      if (!family.queries.has(visit.term.toLowerCase()))
        family.wordings.push(search)
      family.queries.add(visit.term.toLowerCase())
      family.days.add(key)
      family.latest = Math.max(family.latest, visit.at)
    } else {
      families.push({
        anchor: visit.term,
        wordings: [search],
        queries: new Set([visit.term.toLowerCase()]),
        members: 1,
        days: new Set([key]),
        latest: visit.at,
      })
    }
  }
  return families
}
