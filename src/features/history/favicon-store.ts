/**
 * Stored favicons for visible rows. Lookups are batched, cached per site and
 * never block rendering: rows paint a letter badge first and swap the icon in
 * when it arrives. A row with a visit looks its icon up through the visit
 * (`load_history_favicons`); a row that is only a site, as on Insights, asks
 * for the site's own icon (`load_site_favicons`).
 *
 * Responsible for: the per-site cache and batching both lookups.
 * Not responsible for: deciding which rows are visible (virtualized rows
 * request on mount).
 */
import { useSyncExternalStore } from 'react'
import { explorerClient } from '@/lib/backend-client/explorer'
import type { HistoryFaviconLookupEntry } from '@/lib/types'

const BATCH = 40
const FLUSH_DELAY_MS = 60

/** Site → data URL, or null when the archive has no icon for it. */
const cache = new Map<string, string | null>()
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
  for (const [domain] of batch) {
    queued.delete(domain)
    inflight.add(domain)
  }
  const byRequest = new Map(
    batch.map(([domain, entry]) => [
      `${entry.profileId}|${entry.url}|${entry.visitTime}`,
      domain,
    ]),
  )
  try {
    const results = await explorerClient.loadHistoryFavicons(
      batch.map(([, entry]) => entry),
    )
    for (const result of results) {
      const domain = byRequest.get(
        `${result.profileId}|${result.url}|${result.visitTime}`,
      )
      if (domain) cache.set(domain, result.favicon?.dataUrl ?? null)
    }
  } catch {
    // A failed lookup just leaves the letter badge in place.
  }
  for (const [domain] of batch) {
    inflight.delete(domain)
    if (!cache.has(domain)) cache.set(domain, null)
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
    inflight.add(domain)
  }
  try {
    for (const result of await explorerClient.loadSiteFavicons(batch))
      cache.set(result.domain, result.favicon?.dataUrl ?? null)
  } catch {
    // A failed lookup just leaves the letter badge in place.
  }
  for (const domain of batch) {
    inflight.delete(domain)
    if (!cache.has(domain)) cache.set(domain, null)
  }
  notify()
  if (sitesQueued.size > 0)
    sitesTimer ??= setTimeout(() => void flushSites(), FLUSH_DELAY_MS)
}

/** Asks for a site's own icon, for a row that has no visit to look it up through. */
export function requestSiteFavicon(domain: string) {
  if (
    cache.has(domain) ||
    queued.has(domain) ||
    sitesQueued.has(domain) ||
    inflight.has(domain)
  )
    return
  sitesQueued.add(domain)
  sitesTimer ??= setTimeout(() => void flushSites(), FLUSH_DELAY_MS)
}

export function requestFavicon(
  domain: string,
  entry: HistoryFaviconLookupEntry,
) {
  if (cache.has(domain) || queued.has(domain) || inflight.has(domain)) return
  queued.set(domain, entry)
  schedule()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The stored icon for a site, `null` when there is none, `undefined` while unknown. */
export function useFavicon(domain: string) {
  return useSyncExternalStore(subscribe, () => cache.get(domain))
}
