/**
 * Freeing browsing history's semantic search index while semantic search is
 * off (`preview_free_semantic_index`, `free_semantic_index`). The index stays
 * on disk when semantic search is turned off (4–59 GB at 14.4M visits). It is
 * freed from two places, which share what is here: Free… under the semantic
 * search switch in Settings › AI (`kept-browsing-index.tsx`) and its row in
 * Free up space (`free-space-dialog.tsx`).
 *
 * Responsibilities: the commands and words of that Free…, what each refusal
 * means, and what is read again after a free. Not responsible for: whether
 * semantic search is on (`useSemanticIndex`), the other things Free up space
 * deletes, or building the index again (turning semantic search on).
 */
import { appClient } from '@/lib/backend-client/app'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import type { MessageKey } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import { aiQueueKey } from './background/use-background-work'
import type { KeptIndexSource } from './kept-index'

/** Why the backend wouldn't free the index, said the same way wherever it's freed. */
export const INDEX_REFUSED: Record<string, MessageKey> = {
  // An index build or clear is queued or running, or another archive task
  // (a backup, an import, a move) holds the archive.
  'archive-busy': 'settingsAi.semantic.kept.busy',
  // Semantic search is on again, from another window: the index is in use.
  'task-action-unavailable': 'settingsAi.semantic.kept.inUse',
}

/**
 * Reads again what shows the index's size: the AI status in the snapshot
 * (Settings › AI, the model switch prompt, Free up space), Storage's disk
 * usage and the index queue.
 */
export async function refreshAfterIndexFreed() {
  const shown = queryClient.getQueryData(queryKeys.snapshot)
  const current = await appClient.getSnapshot()
  // A save answered meanwhile shows settings at least as new as this read.
  if (queryClient.getQueryData(queryKeys.snapshot) === shown)
    queryClient.setQueryData(queryKeys.snapshot, current)
  void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard })
  void queryClient.invalidateQueries({ queryKey: aiQueueKey })
}

/** Free… in Settings › AI: the browsing index's commands and words. */
export const BROWSING_INDEX: KeptIndexSource = {
  name: 'semantic-index',
  previewFree: intelligenceClient.previewFreeSemanticIndex,
  free: intelligenceClient.freeSemanticIndex,
  onFreed: () => void refreshAfterIndexFreed().catch(() => undefined),
  refused: INDEX_REFUSED,
  words: {
    line: 'settingsAi.semantic.kept.line',
    free: 'settingsAi.semantic.kept.free',
    freeLabel: 'settingsAi.semantic.kept.freeLabel',
    title: 'settingsAi.semantic.kept.title',
    reading: 'settingsAi.semantic.kept.reading',
    body: 'settingsAi.semantic.kept.body',
    nothing: 'settingsAi.semantic.kept.nothing',
    changed: 'settingsAi.semantic.kept.changed',
    confirm: 'settingsAi.semantic.kept.confirm',
    freeing: 'settingsAi.semantic.kept.freeing',
    done: 'settingsAi.semantic.kept.done',
    readFailed: 'settingsAi.semantic.kept.readFailed',
    failed: 'settingsAi.semantic.kept.failed',
  },
}
