/**
 * The line above search results: how much matched and in which mode, or why
 * the regex was not sent. The count is "N+ pages" while the search is still
 * counting and exact once it has counted every match; "still ranking" stays
 * until the order of the rows is final.
 */
import { TriangleAlert } from 'lucide-react'
import { useI18n } from '@/lib/i18n'
import type { SearchSpec } from './history-params'
import type { VisitList } from './queries'
import type { RegexProblem } from './regex-dialect'

/** A plain explanation of why the backend's regex dialect refuses a pattern. */
export function RegexProblemText({ problem }: { problem: RegexProblem }) {
  const { t } = useI18n()
  return (
    <span className="flex items-start gap-1.5 text-destructive">
      <TriangleAlert className="mt-px size-3.5 shrink-0" aria-hidden />
      <span>
        <span className="font-medium">{t('history.modes.invalidRegex')}</span>
        {' · '}
        {problem.kind === 'escape'
          ? t('historySearch.regex.escape', { escape: problem.escape })
          : t(`historySearch.regex.${problem.kind}`)}
      </span>
    </span>
  )
}

export function ResultsLine({
  search,
  list,
}: {
  search: SearchSpec
  list: VisitList
}) {
  const { t } = useI18n()
  if (search.regexError) return <RegexProblemText problem={search.regexError} />

  // The rows on screen are the previous list, kept while the new one starts;
  // a count next to them would describe rows that are not there yet.
  if (list.isPlaceholder) return null
  const status = list.search
  if (!status)
    return list.isPending ? <>{t('history.results.searching')}</> : null

  const mode = t(`history.modes.${search.mode}`)
  // Until the count is exact it is a lower bound ("N+ pages").
  const line = !status.pages.exact
    ? t('history.results.pagesMore', { count: status.pages.value, mode })
    : status.visits?.exact
      ? t('history.results.pagesAndVisits', {
          pages: t('common.pages', { count: status.pages.value }),
          visits: t('common.visits', { count: status.visits.value }),
          mode,
        })
      : t('history.results.pages', {
          pages: t('common.pages', { count: status.pages.value }),
          mode,
        })
  return (
    <>
      {line}
      {status.sorting && ` · ${t('history.results.sorting')}`}
    </>
  )
}
