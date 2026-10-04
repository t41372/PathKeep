/**
 * Cached link-preview images (og:image) for the pages on screen.
 *
 * Responsible for: batching `load_history_og_images`, a size-capped per-URL
 * cache, the one-page on-demand fetch the detail panel asks for
 * (`trigger_og_image_refetch`), and reporting which images were actually
 * shown (`mark_og_images_shown`, the cache's eviction signal).
 * Not responsible for: deciding whether fetching is allowed (the caller reads
 * the setting) or how a preview looks.
 *
 * Images arrive as data URLs of up to a few hundred KB each, so batches are
 * small and the cache keeps only ~120 entries, never evicting one that a
 * mounted component is showing.
 */
import { useEffect, useSyncExternalStore } from 'react'
import { explorerClient } from '@/lib/backend-client/explorer'
import { useSnapshot } from '@/lib/queries/app'

const BATCH = 8
const FLUSH_DELAY_MS = 60
const CACHE_LIMIT = 120
const SHOWN_FLUSH_MS = 2_000

/**
 * What the archive holds for one page. `pending` means no fetch has been
 * tried yet; the other statuses mirror `og_images.fetch_status`
 * (`ok`, `missing`, `blocked`, `http_error`, …).
 */
export interface OgImageEntry {
  status: string
  dataUrl: string | null
}

/** `undefined` while the lookup is in flight; `fetching` while the detail panel downloads one. */
export type OgImageState = OgImageEntry | 'fetching' | undefined

const cache = new Map<string, OgImageEntry>()
const queued = new Set<string>()
const inflight = new Set<string>()
const fetching = new Set<string>()
/** Pages the on-demand fetch already ran for this session; never asked twice. */
const fetched = new Set<string>()
const refs = new Map<string, number>()
const listeners = new Set<() => void>()
const shown = new Set<string>()
let timer: ReturnType<typeof setTimeout> | undefined
let shownTimer: ReturnType<typeof setTimeout> | undefined

function notify() {
  for (const listener of listeners) listener()
}

/** Drops the oldest entries nobody is showing once the cache is over its limit. */
function evict() {
  if (cache.size <= CACHE_LIMIT) return
  for (const url of cache.keys()) {
    if (cache.size <= CACHE_LIMIT) break
    if (!refs.get(url)) cache.delete(url)
  }
}

async function flush() {
  timer = undefined
  const batch = [...queued].slice(0, BATCH)
  if (batch.length === 0) return
  for (const url of batch) {
    queued.delete(url)
    inflight.add(url)
  }
  try {
    const results = await explorerClient.loadHistoryOgImages(
      batch.map((url) => ({ url })),
    )
    for (const result of results) {
      cache.set(result.url, {
        status: result.fetchStatus,
        dataUrl: result.ogImage?.dataUrl ?? null,
      })
    }
  } catch {
    // A failed read leaves the page without a preview, like `missing`.
  }
  for (const url of batch) {
    inflight.delete(url)
    if (!cache.has(url)) cache.set(url, { status: 'unknown', dataUrl: null })
  }
  evict()
  notify()
  if (queued.size > 0) schedule()
}

function schedule() {
  timer ??= setTimeout(() => void flush(), FLUSH_DELAY_MS)
}

function request(url: string) {
  if (cache.has(url) || queued.has(url) || inflight.has(url)) return
  queued.add(url)
  schedule()
}

/**
 * Downloads one page's preview now, then reads the outcome back. Runs at
 * most once per page per session; `retry` asks again after a failure.
 */
export async function fetchOgImage(url: string, { retry = false } = {}) {
  if (fetching.has(url) || (fetched.has(url) && !retry)) return
  fetched.add(url)
  fetching.add(url)
  notify()
  try {
    await explorerClient.triggerOgImageRefetch([url])
  } catch {
    // The read below reports whatever the archive holds.
  }
  fetching.delete(url)
  cache.delete(url)
  request(url)
  notify()
}

/** Records that a preview was on screen, in batches; the cache's LRU cleanup reads it. */
export function markOgImageShown(url: string) {
  shown.add(url)
  shownTimer ??= setTimeout(() => {
    shownTimer = undefined
    const urls = [...shown]
    shown.clear()
    void explorerClient.markOgImagesShown(urls).catch(() => undefined)
  }, SHOWN_FLUSH_MS)
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The cached preview for a page (null skips the lookup), asking for it on mount. */
export function useOgImage(url: string | null): OgImageState {
  useEffect(() => {
    if (!url) return
    refs.set(url, (refs.get(url) ?? 0) + 1)
    request(url)
    return () => {
      const count = (refs.get(url) ?? 1) - 1
      if (count > 0) refs.set(url, count)
      else refs.delete(url)
    }
  }, [url])
  return useSyncExternalStore(subscribe, () => {
    if (!url) return undefined
    if (fetching.has(url)) return 'fetching'
    return cache.get(url)
  })
}

/** Whether the on-demand fetch already ran for this page in this session. */
export function wasFetched(url: string) {
  return fetched.has(url)
}

/** Whether PathKeep may download previews (Settings → General → Link previews). */
export function usePreviewFetching() {
  const og = useSnapshot().config.ogImage
  return (og?.fetchEnabled ?? true) && (og?.fetchMode ?? 'background') !== 'off'
}
