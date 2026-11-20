/**
 * The last row of Records' semantic results while the list isn't whole: a
 * passage-shaped placeholder while the next page loads; after Stop, a way
 * to go on; after a page that failed or was refused for a reason that can
 * pass, what went wrong and Retry, with the passages above kept. The same
 * row stands alone when the first page was stopped.
 *
 * Responsibilities: drawing that row. Not responsible for: when it shows
 * (`semantic-results.tsx`), what Retry or going on does
 * (`use-semantic-search.ts`), or the status line above the list.
 */
import { TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type { RefusalWords } from '@/features/ai-setup/ai-refusals'
import { useI18n } from '@/lib/i18n'
import { PlaceLinks } from './semantic-blocked'
import type { SemanticState } from './use-semantic-search'

export function SemanticListEnd({
  state,
  words,
  onRetry,
}: {
  state: SemanticState
  /** The refusal's words, when the page was refused. */
  words: RefusalWords | null
  /** The same page again (or, after Stop, on from where it stopped). */
  onRetry: () => void
}) {
  const { t } = useI18n()
  if (state.phase === 'loading')
    return (
      <div
        role="status"
        aria-label={t('agentSearch.semantic.more.loading')}
        className="flex flex-col gap-1.5 border-b px-4 py-3"
      >
        <Skeleton className="h-3.5 w-3/5" />
        <Skeleton className="h-3 w-2/5" />
        <Skeleton className="h-3 w-full" />
      </div>
    )
  if (state.phase === 'stopped')
    return (
      <div className="animate-rise flex justify-center px-4 py-4">
        <Button size="sm" variant="outline" onClick={onRetry}>
          {t('agentSearch.semantic.more.resume')}
        </Button>
      </div>
    )
  const title = words?.title ?? t('agentSearch.semantic.more.failed')
  const detail =
    words?.body ??
    (state.error && state.error.message !== state.error.code
      ? state.error.message
      : null)
  return (
    <div
      role="alert"
      className="animate-rise flex flex-col items-center gap-2 px-6 py-4 text-center"
    >
      <p className="flex items-center gap-1.5 text-[13px] font-medium">
        <TriangleAlert
          className="size-4 shrink-0 text-muted-foreground/70"
          aria-hidden
        />
        {title}
      </p>
      {detail && (
        <p className="max-w-[320px] text-[12px] break-words text-muted-foreground">
          {detail}
        </p>
      )}
      <div className="flex flex-wrap justify-center gap-2">
        {words && <PlaceLinks places={words.places} />}
        <Button size="sm" variant="outline" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      </div>
    </div>
  )
}
