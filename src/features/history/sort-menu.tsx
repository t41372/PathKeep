/**
 * The order of search results: best match, newest or oldest first, most
 * visited. Orders
 * the current search mode can't give are listed but disabled, with a line
 * saying why, so the menu never looks broken in regex or semantic mode.
 *
 * Responsible for: showing the orders and reporting the pick.
 * Not responsible for: which orders a mode offers (`sortsFor` in
 * history-params.ts) or remembering the pick (the URL's `sort`).
 */
import { ArrowDownUp, ChevronDown } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useI18n } from '@/lib/i18n'
import {
  searchSorts,
  sortsFor,
  type SearchMode,
  type SearchSort,
} from './history-params'

export function SortMenu({
  mode,
  sort,
  onSort,
}: {
  mode: SearchMode
  sort: SearchSort
  onSort: (sort: SearchSort) => void
}) {
  const { t } = useI18n()
  const offered = sortsFor(mode)
  const label = t(`historySearch.sort.${sort}`)
  const note =
    mode === 'regex'
      ? t('historySearch.sort.regexNote')
      : mode === 'semantic'
        ? t('historySearch.sort.semanticNote')
        : null
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          aria-label={t('historySearch.sort.label', { order: label })}
          className="h-7 animate-rise gap-1.5 px-2 text-xs font-medium text-muted-foreground"
        >
          <ArrowDownUp className="size-3.5" aria-hidden />
          {label}
          <ChevronDown className="size-3.5" aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[240px]">
        <DropdownMenuRadioGroup
          value={sort}
          onValueChange={(next) => onSort(next as SearchSort)}
        >
          {searchSorts.map((option) => (
            <DropdownMenuRadioItem
              key={option}
              value={option}
              disabled={!offered.includes(option)}
              className="items-start [&>span:first-child]:top-[9px]"
            >
              <span className="flex flex-col gap-0.5">
                <span>{t(`historySearch.sort.${option}`)}</span>
                <span className="text-xs text-muted-foreground">
                  {t(`historySearch.sort.about.${option}`)}
                </span>
              </span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
        {note && (
          <p className="border-t px-2 pt-2 pb-1 text-xs leading-relaxed text-muted-foreground">
            {note}
          </p>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
