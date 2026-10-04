/**
 * The line above search results: how much matched and in which mode, or why
 * the regex was not sent. Counts start as "N+ pages" from what is loaded and
 * become exact when the separate count lands (or every page is in).
 */
import { TriangleAlert } from 'lucide-react'
import { useI18n } from '@/lib/i18n'
import type { SearchSpec } from './history-params'
import type { VisitItem } from './history-types'
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
  items,
}: {
  search: SearchSpec
  list: VisitList
  items: VisitItem[]
}) {
  const { t } = useI18n()
  if (search.regexError) return <RegexProblemText problem={search.regexError} />

  // The rows on screen are the previous list, kept while the new one loads;
  // a count next to them would describe rows that are not there yet.
  if (list.isPlaceholder) return null
  const modeLabel = t(`history.modes.${search.mode}`)
  // Until the count lands, describe what is loaded: exact once every page is in.
  const totals =
    list.totals ??
    (list.isPending || items.length === 0 || list.hasNextPage
      ? null
      : {
          pages: items.length,
          visits:
            search.mode === 'semantic'
              ? null
              : items.reduce((sum, item) => sum + (item.visitCount ?? 0), 0),
        })
  if (!totals) {
    return items.length > 0 ? (
      <>
        {t('history.results.pagesMore', {
          count: items.length,
          mode: modeLabel,
        })}
      </>
    ) : null
  }
  const pages = t('common.pages', { count: totals.pages })
  return (
    <>
      {totals.visits === null
        ? t('history.results.pages', { pages, mode: modeLabel })
        : t('history.results.pagesAndVisits', {
            pages,
            visits: t('common.visits', { count: totals.visits }),
            mode: modeLabel,
          })}
    </>
  )
}
