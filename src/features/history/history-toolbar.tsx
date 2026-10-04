/**
 * The History header: search box, view switch, filter row and search-mode
 * toggle. Controlled by the page; holds no data of its own.
 */
import { ChevronDown, Globe, Search, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { DateFilterButton } from './date-filter'
import type { HistoryView, SearchMode } from './history-params'
import type { BrowserOption } from './queries'
import { SearchHelp } from './search-help'

const shortcut =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)
    ? '⌘K'
    : 'Ctrl K'

interface Props {
  draft: string
  onDraft: (value: string) => void
  view: HistoryView
  onView: (view: HistoryView) => void
  starredCount: number | null
  date: string | null
  onDate: (value: string | null) => void
  browser: string | null
  browsers: BrowserOption[]
  onBrowser: (kind: string | null) => void
  domain: string | null
  onClearDomain: () => void
  hasFilters: boolean
  onClearFilters: () => void
  searching: boolean
  mode: SearchMode
  onMode: (mode: SearchMode) => void
  semanticAvailable: boolean
  /** Why semantic search is unavailable, for the disabled toggle's tooltip. */
  semanticOffReason: string | null
  invalidRegex: boolean
  /** A cheat-sheet example was picked: put it in the search box. */
  onExample: (query: string) => void
}

function BrowserFilter({
  value,
  options,
  onChange,
}: {
  value: string | null
  options: BrowserOption[]
  onChange: (kind: string | null) => void
}) {
  const { t } = useI18n()
  const selected = options.find((option) => option.kind === value)
  const label = selected?.name ?? t('history.browser.all')
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          aria-label={`${t('history.browser.label')}: ${label}`}
          className={cn(
            'h-[30px] bg-card px-2.5 font-normal',
            value && 'border-foreground',
          )}
        >
          {selected ? (
            <BrowserIcon
              browserName={selected.name}
              className="size-3.5"
              decorative
            />
          ) : (
            <Globe className="size-3.5" />
          )}
          {label}
          <ChevronDown className="size-3.5 text-muted-foreground" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-[200px]">
        <DropdownMenuRadioGroup
          value={value ?? 'all'}
          onValueChange={(next) => onChange(next === 'all' ? null : next)}
        >
          <DropdownMenuRadioItem value="all">
            {t('history.browser.all')}
          </DropdownMenuRadioItem>
          {options.map((option) => (
            <DropdownMenuRadioItem key={option.kind} value={option.kind}>
              <BrowserIcon
                browserName={option.name}
                className="size-4"
                decorative
              />
              {option.name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function ModeToggle({
  mode,
  onMode,
  semanticAvailable,
  semanticOffReason,
}: Pick<Props, 'mode' | 'onMode' | 'semanticAvailable' | 'semanticOffReason'>) {
  const { t } = useI18n()
  const item =
    'h-7 rounded-full border px-2.5 text-xs font-medium text-muted-foreground data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground'
  return (
    <ToggleGroup
      type="single"
      value={mode}
      onValueChange={(next) => next && onMode(next as SearchMode)}
      spacing={1}
      aria-label={t('history.modes.label')}
      className="ml-auto animate-rise"
    >
      <ToggleGroupItem value="full" className={item}>
        {t('history.modes.full')}
      </ToggleGroupItem>
      <ToggleGroupItem value="regex" className={item}>
        {t('history.modes.regex')}
      </ToggleGroupItem>
      <Tooltip>
        <TooltipTrigger asChild>
          {/* A span, because a disabled button never fires the pointer events a tooltip needs. */}
          <span tabIndex={semanticAvailable ? -1 : 0}>
            <ToggleGroupItem
              value="semantic"
              disabled={!semanticAvailable}
              className={item}
            >
              {t('history.modes.semantic')}
            </ToggleGroupItem>
          </span>
        </TooltipTrigger>
        {!semanticAvailable && (
          <TooltipContent className="max-w-[260px]">
            {semanticOffReason} {t('historySearch.semantic.whereToFix')}
          </TooltipContent>
        )}
      </Tooltip>
    </ToggleGroup>
  )
}

export function HistoryToolbar(props: Props) {
  const { t } = useI18n()
  const format = useFormat()
  const { draft, onDraft, view, onView, starredCount } = props

  return (
    <div className="flex flex-col gap-3 border-b px-7 pt-5 pb-3.5">
      <div className="flex flex-wrap items-center gap-2.5">
        <div className="relative flex min-w-[260px] flex-1 items-center">
          <Search className="pointer-events-none absolute left-3 size-4 text-muted-foreground" />
          <Input
            type="search"
            value={draft}
            onChange={(event) => onDraft(event.target.value)}
            placeholder={t('history.search.placeholder')}
            aria-label={t('history.search.label')}
            aria-invalid={props.invalidRegex || undefined}
            className="h-10 rounded-[10px] bg-card pr-[140px] pl-[38px] shadow-card [&::-webkit-search-cancel-button]:hidden"
          />
          <div className="absolute right-2.5 flex items-center gap-1.5">
            {draft && (
              <button
                type="button"
                aria-label={t('history.search.clear')}
                onClick={() => onDraft('')}
                className="flex size-[22px] items-center justify-center rounded-full bg-muted text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                <X className="size-3" />
              </button>
            )}
            <SearchHelp onPick={props.onExample} />
            <kbd className="pointer-events-none rounded-[5px] border px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground">
              {shortcut}
            </kbd>
          </div>
        </div>
        <Tabs
          value={view}
          onValueChange={(next) => onView(next as HistoryView)}
        >
          <TabsList
            aria-label={t('history.views.label')}
            className="h-[38px] rounded-[9px] gap-0.5"
          >
            {(['timeline', 'sites', 'starred'] as const).map((name) => (
              <TabsTrigger
                key={name}
                value={name}
                className="h-8 rounded-[7px] px-3 text-[13px]"
              >
                {t(`history.views.${name}`)}
                {name === 'starred' &&
                  starredCount !== null &&
                  starredCount > 0 && (
                    <span className="font-mono text-[11px] text-muted-foreground">
                      {format.number(starredCount)}
                    </span>
                  )}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <DateFilterButton value={props.date} onChange={props.onDate} />
        <BrowserFilter
          value={props.browser}
          options={props.browsers}
          onChange={props.onBrowser}
        />
        {props.domain && (
          <span className="flex h-[30px] animate-rise items-center gap-1.5 rounded-lg bg-primary pr-1.5 pl-2.5 text-[13px] text-primary-foreground">
            {props.domain}
            <button
              type="button"
              aria-label={t('history.filters.removeSite', {
                domain: props.domain,
              })}
              onClick={props.onClearDomain}
              className="flex size-5 items-center justify-center rounded outline-none hover:bg-primary-foreground/15 focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <X className="size-3.5" />
            </button>
          </span>
        )}
        {props.hasFilters && (
          <Button
            variant="ghost"
            size="sm"
            className="h-[30px] px-2 font-normal text-muted-foreground"
            onClick={props.onClearFilters}
          >
            {t('history.filters.clear')}
          </Button>
        )}
        {props.searching && (
          <ModeToggle
            mode={props.mode}
            onMode={props.onMode}
            semanticAvailable={props.semanticAvailable}
            semanticOffReason={props.semanticOffReason}
          />
        )}
      </div>
    </div>
  )
}
