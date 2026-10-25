/**
 * How the detail panel's note and tag editors write a page's annotation.
 *
 * Saves of one field of one page go one after another, across every editor
 * that page has had: leaving the page keeps the editor's last save waiting,
 * and coming back opens a new editor. With a queue each, the new editor's
 * save could reach the archive first and the old one land after it.
 *
 * A save's answer carries the whole annotation as the archive had it then;
 * only the field the save wrote goes into the cache. The other field may
 * have been saved since, and an editor that took the older value would write
 * it back with its next save.
 *
 * Not responsible for: the editors' state, or reading the annotation.
 */
import type { QueryClient } from '@tanstack/react-query'
import type { UrlAnnotation } from '@/lib/backend-client/annotations'

export type AnnotationField = 'notes' | 'tags'

const queues = new Map<string, Promise<void>>()

/** Runs `work` after every save of `field` for `url` asked for before it. */
export function enqueueSave(
  field: AnnotationField,
  url: string,
  work: () => Promise<void>,
): Promise<void> {
  const key = `${field}\n${url}`
  const next = (queues.get(key) ?? Promise.resolve()).then(work, work)
  queues.set(key, next)
  const forget = () => {
    if (queues.get(key) === next) queues.delete(key)
  }
  void next.then(forget, forget)
  return next
}

/**
 * The page's note as the archive has it after every save asked for so far:
 * the cache each save writes into. A save waiting in the queue compares with
 * this when it runs, not with what its own editor saw: another editor of the
 * page, open earlier, may have saved since.
 */
export function cachedNotes(client: QueryClient, url: string) {
  return client.getQueryData<UrlAnnotation>(['annotations', 'url', url])?.notes
}

/** Puts the field a save wrote into the page's cached annotation. */
export function cacheSavedField(
  client: QueryClient,
  url: string,
  field: AnnotationField,
  saved: UrlAnnotation,
) {
  client.setQueryData<UrlAnnotation>(['annotations', 'url', url], (cached) =>
    cached
      ? { ...cached, [field]: saved[field], updatedAt: saved.updatedAt }
      : saved,
  )
}
