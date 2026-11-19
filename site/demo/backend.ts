/**
 * The demo's backend: answers the real backend gave, recorded over a
 * synthetic archive and replayed here. Nothing is saved: a change goes
 * through as far as the app can see and is kept nowhere (`writes.ts`).
 *
 * Responsible for: answering a command from the recording, in this order:
 * a batch lookup item by item; the exact call (opening the file of the day
 * or site it belongs to, see `shards.ts`); a History list worked out from
 * every recorded visit (`timeline.ts`); a page opened in a new tab; a
 * change (`writes.ts`); an Insights card's section from the batch it came
 * in; the same command with the closest arguments;
 * otherwise the demo's refusal. Every app snapshot says the archive's
 * owner kept Insights in its own time zone on this computer. Not
 * responsible for: how the app reaches it (`transport/`), or making the
 * recording (`record.ts`).
 */
import type {
  SearchEvent,
  SearchHit,
  SearchRequest,
  SearchStarted,
  SearchStep,
} from '@/lib/backend-client/search'
import { readStoredLanguagePreference, resolveLanguage } from '@/lib/i18n'
import { CommandInvokeError } from '@/lib/ipc/command-error'
import { computerTimeZone } from '@/lib/time-zone'
import type {
  AppSnapshot,
  HistoryEntry,
  HistoryQuery,
  HistoryQueryResponse,
} from '@/lib/types'
import { shardOf } from './shards'
import { unpackTimeline, type PackedTimeline } from './timeline'
import { writes, type Recorded } from './writes'

/** One command the app sent while recording, and what came back. */
export interface RecordedCall {
  command: string
  args: unknown
  status: number
  response: unknown
}

/** One search the app ran while recording, and the events it streamed. */
export interface RecordedSearch {
  request: SearchRequest
  started: SearchStarted
  events: SearchEvent[]
}

export interface Recording {
  /** When the recording was made; the demo's clock starts here. */
  recordedAt: string
  /** The time zone it was made in (the archive's), as an IANA name. */
  zone: string
  calls: RecordedCall[]
  searches: RecordedSearch[]
  /** Every page the search index held, each with its latest visit. */
  pages: SearchHit[]
}

const refusal = {
  en: 'The demo can’t do this: it runs in your browser and saves nothing.',
  'zh-TW': 'demo 做不到這件事：它在瀏覽器裡跑，什麼都不會儲存。',
  'zh-CN': 'demo 做不到这件事：它在浏览器里运行，什么都不会保存。',
}

/** JSON with object keys sorted, so equal arguments give equal keys. */
export function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
  if (value && typeof value === 'object')
    return `{${Object.keys(value)
      .filter((k) => (value as Record<string, unknown>)[k] !== undefined)
      .sort()
      .map(
        (k) =>
          `${JSON.stringify(k)}:${canonical((value as Record<string, unknown>)[k])}`,
      )
      .join(',')}}`
  return JSON.stringify(value ?? null)
}

const keyOf = (command: string, args: unknown) =>
  `${command} ${canonical(args)}`

/**
 * Commands that look up a batch of items, one answer per item, for the rows
 * on screen. Which rows those are depends on where the visitor scrolled, so
 * each item is answered from every item the recording looked up.
 */
type Lookup = {
  items: (args: Record<string, unknown>) => unknown[]
  key: (item: unknown) => string
  /** Builds the reply from the recorded answers, by item key. */
  reply: (items: unknown[], known: Map<string, unknown>) => unknown
  /** Each recorded answer's items, keyed like the request's. */
  learn: (
    args: Record<string, unknown>,
    response: unknown,
  ) => [string, unknown][]
}
type Entry = { url: string }
const perItem = (field: string, fallback: (item: Entry) => object): Lookup => ({
  items: (args) => (args[field] as unknown[]) ?? [],
  key: (item) => (item as Entry).url,
  learn: (_, response) =>
    (response as Entry[]).map((answer) => [answer.url, answer]),
  reply: (items, known) =>
    (items as Entry[]).map((item) => ({
      ...fallback(item),
      ...(known.get(item.url) as object),
      ...item,
    })),
})
const lookups: Record<string, Lookup> = {
  load_history_favicons: perItem('entries', () => ({ favicon: null })),
  // The recording ran offline; the demo shows every page without a preview
  // image rather than "blocked".
  load_history_og_images: {
    items: (args) => (args.entries as unknown[]) ?? [],
    key: (item) => (item as Entry).url,
    learn: () => [],
    reply: (items) =>
      (items as Entry[]).map(({ url }) => ({
        url,
        ogImage: null,
        fetchStatus: 'missing',
      })),
  },
  load_site_favicons: {
    items: (args) => (args.domains as string[]) ?? [],
    key: (domain) => domain as string,
    learn: (_, response) =>
      (response as { domain: string }[]).map((answer) => [
        answer.domain,
        answer,
      ]),
    reply: (domains, known) =>
      (domains as string[]).map(
        (domain) => known.get(domain) ?? { domain, favicon: null },
      ),
  },
  get_star_status: {
    items: (args) =>
      ((args.request as { entityKeys?: string[] })?.entityKeys ??
        []) as unknown[],
    key: (key) => key as string,
    learn: (_, response) => Object.entries(response as Record<string, boolean>),
    reply: (keys, known) =>
      Object.fromEntries(
        (keys as string[]).map((key) => [key, known.get(key) ?? false]),
      ),
  },
}

const exact = new Map<string, RecordedCall>()
const byCommand = new Map<string, RecordedCall[]>()
const known = new Map<string, Map<string, unknown>>()
const searches = new Map<string, RecordedSearch>()
let pages: SearchHit[] = []
let lastStep: SearchStep | null = null
/** Every visit, newest first; read from `recording/timeline.json` when first needed. */
let timeline: HistoryEntry[] | undefined
/** How a recorded word search (⌘K) said the index stood. */
let searchState: HistoryQueryResponse['searchState'] = null

export function takeRecording(recording: Recording) {
  for (const call of recording.calls) {
    exact.set(keyOf(call.command, call.args), call)
    byCommand.set(call.command, [...(byCommand.get(call.command) ?? []), call])
    const lookup = lookups[call.command]
    if (lookup && call.status < 400) {
      const map = known.get(call.command) ?? new Map<string, unknown>()
      for (const [key, value] of lookup.learn(
        call.args as Record<string, unknown>,
        call.response,
      ))
        map.set(key, value)
      known.set(call.command, map)
    }
  }
  for (const search of recording.searches ?? []) {
    searches.set(canonical(search.request), search)
    for (const event of search.events)
      if (event.type === 'step' && !event.data.provisional)
        lastStep = event.data
  }
  pages = recording.pages ?? []
  searchState =
    (
      recording.calls.find(
        (call) =>
          call.command === 'query_history' &&
          (call.response as HistoryQueryResponse)?.searchState,
      )?.response as HistoryQueryResponse | undefined
    )?.searchState ?? null
}

/** The recording's files, each fetched once, when first needed. */
const opened = new Map<string, Promise<unknown>>()
function open<T>(name: string): Promise<T | null> {
  if (!opened.has(name))
    opened.set(
      name,
      fetch(`./recording/${name}.json`).then((response) =>
        response.ok ? response.json() : null,
      ),
    )
  return opened.get(name) as Promise<T | null>
}

/** Takes in a day's or a site's file the first time one of its calls is asked for. */
async function openShard(command: string, args: unknown) {
  const shard = shardOf(command, args)
  if (!shard || opened.has(shard)) {
    if (shard) await opened.get(shard)
    return
  }
  const calls = await open<RecordedCall[]>(shard)
  for (const call of calls ?? []) {
    exact.set(keyOf(call.command, call.args), call)
    byCommand.set(call.command, [...(byCommand.get(call.command) ?? []), call])
  }
}

/** Every visit, newest first, as History lists them. */
async function everyVisit(): Promise<HistoryEntry[]> {
  timeline ??= unpackTimeline(
    (await open<PackedTimeline>('timeline')) ?? {
      pages: [],
      profiles: [],
      apps: [],
      visits: [],
    },
  )
  return timeline
}

/** Every word of `query` in the page's title or address. */
const matchesWords = (query: string) => {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  return (visit: HistoryEntry) => {
    const text = `${visit.title ?? ''} ${visit.url}`.toLowerCase()
    return words.every((word) => text.includes(word))
  }
}

/**
 * A History list the recording has no exact answer for, worked out from
 * what it holds: words (⌘K) over every page; anything else (a date range,
 * a browser, a site, a page further down) over every visit.
 */
async function historyList(
  query: HistoryQuery,
): Promise<HistoryQueryResponse | null> {
  if (query.regexMode) return null
  const kept = (visit: HistoryEntry) =>
    (!query.profileId || visit.profileId === query.profileId) &&
    (!query.browserKind ||
      visit.profileId.startsWith(`${query.browserKind}:`)) &&
    (!query.domain ||
      visit.domain === query.domain ||
      visit.domain.endsWith(`.${query.domain}`)) &&
    (query.startTimeMs == null || visit.visitTime >= query.startTimeMs) &&
    (query.endTimeMs == null || visit.visitTime <= query.endTimeMs)
  let rows: HistoryEntry[]
  if (query.q) {
    if (!query.groupByUrl) return null
    rows = pages
      .map((hit) => hit.visit)
      .filter((visit) => kept(visit) && matchesWords(query.q ?? '')(visit))
    rows.sort(
      (a, b) =>
        (query.sort === 'relevance'
          ? (b.visitCount ?? 0) - (a.visitCount ?? 0)
          : 0) || b.visitTime - a.visitTime,
    )
  } else {
    const visits = await everyVisit()
    if (visits.length === 0) return null
    rows = visits.filter(kept)
  }
  if (query.sort === 'oldest') rows.reverse()
  const cursorOf = (row: HistoryEntry) =>
    query.q ? `demo:${row.url}` : `${row.visitTime}|${row.id}`
  const start = query.cursor
    ? rows.findIndex((row) => cursorOf(row) === query.cursor) + 1
    : 0
  const limit = query.limit ?? 100
  const items = rows.slice(start, start + limit)
  const hasNext = start + limit < rows.length
  return {
    searchState: query.q ? searchState : null,
    total: items.length,
    totalExact: false,
    totalVisits: query.q
      ? items.reduce((sum, row) => sum + (row.visitCount ?? 1), 0)
      : null,
    items,
    page: 1,
    pageSize: limit,
    pageCount: 1,
    hasPrevious: start > 0,
    hasNext,
    nextCursor:
      hasNext && items.length > 0 ? cursorOf(items[items.length - 1]) : null,
  }
}

/** Every leaf of a JSON value, by its path. */
function leaves(value: unknown, at = '', into = new Map<string, unknown>()) {
  if (value && typeof value === 'object')
    for (const [key, inner] of Object.entries(value))
      leaves(inner, `${at}.${key}`, into)
  else into.set(at, value)
  return into
}

const DAY = 86_400_000
const asTime = (value: unknown) =>
  typeof value === 'string' && /^\d{4}-\d{2}-\d{2}/.test(value)
    ? Date.parse(value)
    : typeof value === 'number' && value > 1e12
      ? value
      : null

/**
 * How alike two calls' arguments are: one point per equal leaf, less a
 * little for each day between dates in the same place. A custom date range
 * gets the recorded range closest to it.
 */
function likeness(
  wanted: Map<string, unknown>,
  recorded: Map<string, unknown>,
) {
  let score = 0
  for (const [at, value] of wanted) {
    const other = recorded.get(at)
    if (other === value) score += 1
    else {
      const [a, b] = [asTime(value), asTime(other)]
      if (a !== null && b !== null)
        score -= Math.min(Math.abs(a - b) / DAY, 3650) / 3650
    }
  }
  return score
}

function refuse(command: string): never {
  console.warn('[demo] not recorded:', command)
  const language = resolveLanguage(readStoredLanguagePreference())
  throw new CommandInvokeError(refusal[language] ?? refusal.en, {
    code: 'demo',
  })
}

function replay(call: RecordedCall) {
  if (call.status >= 400) {
    const envelope = (call.response ?? {}) as Record<string, string | null>
    throw new CommandInvokeError(
      envelope.message ?? `"${call.command}" failed.`,
      {
        cause: call.response,
        code: envelope.code,
        actionHint: envelope.actionHint,
        retryHint: envelope.retryHint,
      },
    )
  }
  return structuredClone(call.response)
}

/**
 * Insights read their cards in two batches (`features/insights/overview.ts`);
 * a card reads its own only when its copy went stale, after a backup, say.
 * The recording holds those sections only inside their batch: by the
 * command a card reads with, its batch and the field there.
 */
const inBatch: Record<string, [batch: string, field: string]> = {
  get_digest_summary: ['get_intelligence_primary_overview', 'digestSummary'],
  get_top_sites: ['get_intelligence_primary_overview', 'topSites'],
  get_refind_pages: ['get_intelligence_primary_overview', 'refindPages'],
  get_search_engine_ranking: [
    'get_intelligence_primary_overview',
    'searchEngineRanking',
  ],
  get_top_search_concepts: [
    'get_intelligence_primary_overview',
    'topSearchConcepts',
  ],
  get_query_families: ['get_intelligence_primary_overview', 'queryFamilies'],
  get_activity_mix: ['get_intelligence_primary_overview', 'activityMix'],
  get_discovery_trend: [
    'get_intelligence_primary_overview',
    'discoveryTrendDay',
  ],
  get_habit_patterns: ['get_intelligence_primary_overview', 'habitPatterns'],
  get_stable_sources: ['get_intelligence_secondary_overview', 'stableSources'],
  get_search_effectiveness: [
    'get_intelligence_secondary_overview',
    'searchEffectiveness',
  ],
  get_friction_signals: [
    'get_intelligence_secondary_overview',
    'frictionSignals',
  ],
  get_reopened_investigations: [
    'get_intelligence_secondary_overview',
    'reopenedInvestigations',
  ],
  get_breadth_index: ['get_intelligence_secondary_overview', 'breadthIndex'],
  get_path_flows: ['get_intelligence_secondary_overview', 'pathFlows'],
  get_compare_sets: ['get_intelligence_secondary_overview', 'compareSets'],
  get_multi_browser_diff: [
    'get_intelligence_secondary_overview',
    'multiBrowserDiff',
  ],
}

type CardRequest = {
  dateRange?: unknown
  profileId?: unknown
  cursor?: unknown
  sortBy?: unknown
}

/** The range and profile a card or batch is for, without its page or order. */
const scopeOf = (args: unknown) => {
  const request = (args as { request?: CardRequest } | null)?.request
  return canonical({
    dateRange: request?.dateRange,
    profileId: request?.profileId ?? null,
  })
}

/**
 * A card's section from the batch for the same range: what the backend
 * computed for that card there. Only a first page in the usual order, the
 * one a batch holds.
 */
function fromBatch(command: string, args: unknown): unknown {
  const where = inBatch[command]
  const request = (args as { request?: CardRequest } | null)?.request
  if (!where || !request || request.cursor != null) return undefined
  if (request.sortBy != null && request.sortBy !== 'visits') return undefined
  const [batch, field] = where
  const scope = scopeOf(args)
  const call = byCommand
    .get(batch)
    ?.find(
      (recorded) => recorded.status < 400 && scopeOf(recorded.args) === scope,
    )
  if (!call) return undefined
  return structuredClone((call.response as Record<string, unknown>)[field])
}

/** What the recording says now, for building the answer to a change. */
const recorded: Recorded = {
  latest<T>(command: string) {
    const call = byCommand.get(command)?.at(-1)
    if (!call) refuse(command)
    return replay(call) as T
  },
  exactly<T>(command: string, args: unknown) {
    const call = exact.get(keyOf(command, args))
    return call ? (replay(call) as T) : undefined
  },
}

/** The commands whose answer is the app snapshot, which holds the settings. */
const snapshotCommands = new Set([
  'app_snapshot',
  'save_config',
  'set_content_fetch_settings',
  'store_ai_provider_api_key',
  'clear_ai_provider_api_key',
])

/**
 * The archive's owner kept Insights in its own time zone on the visitor's
 * computer, so the app does not offer to move them: they stay in the zone
 * the recording computed them in (`features/insights/zone.ts`).
 */
const keptZone = (snapshot: AppSnapshot): AppSnapshot => ({
  ...snapshot,
  config: { ...snapshot.config, insightsZoneKeptFor: computerTimeZone() },
})

export async function answer(command: string, args: unknown): Promise<unknown> {
  const reply = await fromRecording(command, args)
  return snapshotCommands.has(command) ? keptZone(reply as AppSnapshot) : reply
}

async function fromRecording(command: string, args: unknown): Promise<unknown> {
  const lookup = lookups[command]
  if (lookup) {
    const items = lookup.items((args ?? {}) as Record<string, unknown>)
    return lookup.reply(items, known.get(command) ?? new Map())
  }

  await openShard(command, args)
  const hit = exact.get(keyOf(command, args))
  if (hit) return replay(hit)

  if (command === 'query_history') {
    const list = await historyList((args as { query: HistoryQuery }).query)
    if (list) {
      console.info(
        '[demo] listed in the page:',
        (args as { query: HistoryQuery }).query,
      )
      return list
    }
  }

  if (command === 'open_visited_page' || command === 'open_external_url') {
    const url = (args as { url?: string } | undefined)?.url
    if (url && /^https?:\/\//.test(url))
      window.open(url, '_blank', 'noopener,noreferrer')
    return ''
  }

  const write = writes[command]
  if (write) return write(args as never, recorded)

  const section = fromBatch(command, args)
  if (section !== undefined) {
    console.info('[demo] taken from its batch:', command)
    return section
  }

  const calls = byCommand.get(command)
  if (!calls) refuse(command)
  const wanted = leaves(args)
  let best = calls[0]
  let bestScore = -Infinity
  for (const call of calls) {
    const score = likeness(wanted, leaves(call.args))
    if (score > bestScore) [best, bestScore] = [call, score]
  }
  console.info('[demo] nearest answer:', command)
  return replay(best)
}

/**
 * A search: the recorded one when the walk typed the same; otherwise the
 * demo searches the recorded pages itself, every word in the title or the
 * address (or the pattern, in regex mode). The ranking is simpler than the
 * app's, the pages are the same.
 */
export function searchFor(request: SearchRequest): RecordedSearch {
  const recorded = searches.get(canonical(request))
  if (recorded) return structuredClone(recorded)
  console.info('[demo] searched in the page:', request.query)
  const query = (request.query ?? '').trim()
  let test: (text: string) => number
  if (request.regex) {
    let pattern: RegExp | null = null
    try {
      pattern = new RegExp(query, 'iu')
    } catch {
      // The app says so itself before it searches.
    }
    test = (text) => (pattern?.test(text) ? 1 : 0)
  } else {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean)
    test = (text) => {
      const lower = text.toLowerCase()
      return words.every((word) => lower.includes(word)) ? words.length : 0
    }
  }
  const hits = pages
    .filter(({ visit }) => {
      if (request.profileId && visit.profileId !== request.profileId)
        return false
      if (
        request.browserKind &&
        !visit.profileId.startsWith(`${request.browserKind}:`)
      )
        return false
      if (request.domain && !visit.domain.endsWith(request.domain)) return false
      if (request.startTimeMs != null && visit.visitTime < request.startTimeMs)
        return false
      if (request.endTimeMs != null && visit.visitTime > request.endTimeMs)
        return false
      return !request.starred
    })
    .map((hit) => ({
      ...hit,
      score: test(hit.visit.title ?? '') * 2 + test(hit.visit.url),
    }))
    .filter((hit) => hit.score > 0)
  const order: Record<string, (a: SearchHit, b: SearchHit) => number> = {
    newest: (a, b) => b.visit.visitTime - a.visit.visitTime,
    oldest: (a, b) => a.visit.visitTime - b.visit.visitTime,
    visits: (a, b) => (b.visit.visitCount ?? 0) - (a.visit.visitCount ?? 0),
    relevance: (a, b) =>
      b.score - a.score ||
      (b.visit.visitCount ?? 0) - (a.visit.visitCount ?? 0),
  }
  hits.sort(order[request.sort ?? 'relevance'] ?? order.relevance)
  const visits = hits.reduce((sum, hit) => sum + (hit.visit.visitCount ?? 1), 0)
  const step = {
    ...(lastStep as SearchStep),
    page: 1,
    items: hits,
    provisional: false,
    count: { value: hits.length, exact: true },
    visits: { value: visits, exact: true },
    cursor: null,
  }
  return {
    request,
    started: { sessionId: `demo-${Date.now()}`, indexer: step.indexer },
    events: [
      { type: 'indexer', data: step.indexer },
      { type: 'step', data: step },
    ],
  }
}
