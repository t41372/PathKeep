/**
 * Stored favicons for visible rows. Lookups are batched, cached and never
 * block rendering: rows paint a letter badge first and swap the icon in when
 * it arrives. A row with a visit looks its icon up through the visit
 * (`load_history_favicons`: the page's icon as stored by then, else its
 * host's), cached per visit; a row that is only a site, as on Insights, asks
 * for the site's own icon (`load_site_favicons`), cached per site.
 *
 * The two are cached apart: a page's icon can be another site's (a redirect,
 * a sign-in page), so it must never stand for the site on Insights. Nor can
 * one page's stand for another page of its site: a host that serves several
 * things (Google's has Maps and Drive) gives each its own.
 *
 * Responsible for: the cache, kept to the most recent lookups, and batching
 * both lookups.
 * Not responsible for: deciding which rows are visible (virtualized rows
 * request on mount).
 */
import { useSyncExternalStore } from 'react'
import { explorerClient } from '@/lib/backend-client/explorer'
import type { HistoryFaviconLookupEntry } from '@/lib/types'

const BATCH = 40
const FLUSH_DELAY_MS = 60

/**
 * Lookups kept. Scrolling History asks for one per row, so the oldest are
 * dropped; rows on screen were asked for last and stay.
 */
const CACHE_LIMIT = 4096

/**
 * Key → data URL, or null when the archive has no icon for it. A visit's
 * lookup is keyed by `visitKey`, a site's own icon by `siteKey`.
 */
const cache = new Map<string, string | null>()
/** One string per distinct icon, so thousands of rows share a few images. */
const images = new Map<string, string>()

function remember(key: string, dataUrl: string | null) {
  let image = dataUrl
  if (image !== null) {
    image = images.get(image) ?? image
    images.set(image, image)
  }
  cache.delete(key)
  cache.set(key, image)
  for (const old of cache.keys()) {
    if (cache.size <= CACHE_LIMIT) break
    cache.delete(old)
  }
}

/** The cache key of a site's own icon, apart from icons found through a visit. */
export const siteKey = (domain: string) => `site:${domain}`
/** The cache key of the icon found through one visit. */
export const visitKey = (entry: HistoryFaviconLookupEntry) =>
  `visit:${entry.profileId}|${entry.url}|${entry.visitTime}`
const queued = new Map<string, HistoryFaviconLookupEntry>()
const inflight = new Set<string>()
const listeners = new Set<() => void>()
let timer: ReturnType<typeof setTimeout> | undefined

function notify() {
  for (const listener of listeners) listener()
}

async function flush() {
  timer = undefined
  const batch = [...queued].slice(0, BATCH)
  if (batch.length === 0) return
  for (const [key] of batch) {
    queued.delete(key)
    inflight.add(key)
  }
  try {
    const results = await explorerClient.loadHistoryFavicons(
      batch.map(([, entry]) => entry),
    )
    for (const result of results)
      remember(visitKey(result), result.favicon?.dataUrl ?? null)
  } catch {
    // A failed lookup just leaves the letter badge in place.
  }
  for (const [key] of batch) {
    inflight.delete(key)
    if (!cache.has(key)) remember(key, null)
  }
  notify()
  if (queued.size > 0) schedule()
}

function schedule() {
  timer ??= setTimeout(() => void flush(), FLUSH_DELAY_MS)
}

const sitesQueued = new Set<string>()
let sitesTimer: ReturnType<typeof setTimeout> | undefined

async function flushSites() {
  sitesTimer = undefined
  const batch = [...sitesQueued].slice(0, BATCH)
  if (batch.length === 0) return
  for (const domain of batch) {
    sitesQueued.delete(domain)
    inflight.add(siteKey(domain))
  }
  try {
    for (const result of await explorerClient.loadSiteFavicons(batch))
      remember(siteKey(result.domain), result.favicon?.dataUrl ?? null)
  } catch {
    // A failed lookup just leaves the letter badge in place.
  }
  for (const domain of batch) {
    inflight.delete(siteKey(domain))
    if (!cache.has(siteKey(domain))) remember(siteKey(domain), null)
  }
  notify()
  if (sitesQueued.size > 0)
    sitesTimer ??= setTimeout(() => void flushSites(), FLUSH_DELAY_MS)
}

/** Asks for a site's own icon, for a row that has no visit to look it up through. */
export function requestSiteFavicon(domain: string) {
  const key = siteKey(domain)
  if (cache.has(key) || sitesQueued.has(domain) || inflight.has(key)) return
  sitesQueued.add(domain)
  sitesTimer ??= setTimeout(() => void flushSites(), FLUSH_DELAY_MS)
}

/** Asks for the icon a row's visit had: the page's own, else its host's. */
export function requestFavicon(entry: HistoryFaviconLookupEntry) {
  const key = visitKey(entry)
  if (cache.has(key) || queued.has(key) || inflight.has(key)) return
  queued.set(key, entry)
  schedule()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/**
 * The stored icon under `key` (`visitKey` or `siteKey`), `null` when there is
 * none, `undefined` while unknown.
 */
export function useFavicon(key: string) {
  return useSyncExternalStore(subscribe, () => cache.get(key))
}
