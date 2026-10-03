/** The date dropdown: presets, plus a calendar for a custom range. */
import { Calendar as CalendarIcon, Check, ChevronDown } from 'lucide-react'
import { useState } from 'react'
import type { DateRange } from 'react-day-picker'
import { enUS, zhCN, zhTW } from 'date-fns/locale'
import { Button } from '@/components/ui/button'
import { Calendar } from '@/components/ui/calendar'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Separator } from '@/components/ui/separator'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  parseDateFilter,
  parseDayKey,
  rangeParam,
  type DateFilter,
} from './history-params'

const presets = [
  { value: null, label: 'history.date.all' },
  { value: 'today', label: 'history.date.today' },
  { value: 'yesterday', label: 'history.date.yesterday' },
  { value: '7d', label: 'history.date.last7' },
  { value: '30d', label: 'history.date.last30' },
] as const

const calendarLocales = { en: enUS, 'zh-CN': zhCN, 'zh-TW': zhTW }

function useDateLabel() {
  const { t } = useI18n()
  const format = useFormat()
  return (filter: DateFilter) => {
    if (filter.kind === 'all') return t('history.date.all')
    if (filter.kind === 'preset') {
      return t(
        {
          today: 'history.date.today',
          yesterday: 'history.date.yesterday',
          '7d': 'history.date.last7',
          '30d': 'history.date.last30',
        }[filter.preset] as 'history.date.today',
      )
    }
    const start = format.date(parseDayKey(filter.start), {
      month: 'short',
      day: 'numeric',
    })
    if (filter.start === filter.end) return start
    return `${start} – ${format.date(parseDayKey(filter.end), { month: 'short', day: 'numeric' })}`
  }
}

export function DateFilterButton({
  value,
  onChange,
}: {
  value: string | null
  onChange: (value: string | null) => void
}) {
  const { t, language } = useI18n()
  const label = useDateLabel()
  const filter = parseDateFilter(value)
  const [open, setOpen] = useState(false)
  const [picking, setPicking] = useState(false)
  const [draft, setDraft] = useState<DateRange | undefined>()

  const choose = (next: string | null) => {
    onChange(next)
    setOpen(false)
  }

  const openCalendar = () => {
    setDraft(
      filter.kind === 'range'
        ? { from: parseDayKey(filter.start), to: parseDayKey(filter.end) }
        : undefined,
    )
    setPicking(true)
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) setPicking(false)
      }}
    >
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          aria-label={`${t('history.date.label')}: ${label(filter)}`}
          className={cn(
            'h-[30px] bg-card px-2.5 font-normal',
            filter.kind !== 'all' && 'border-foreground',
          )}
        >
          <CalendarIcon className="size-3.5" />
          {label(filter)}
          <ChevronDown className="size-3.5 text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className={cn('p-1', picking ? 'w-auto' : 'w-[200px]')}
      >
        {picking ? (
          <div className="flex flex-col">
            <Calendar
              mode="range"
              selected={draft}
              onSelect={setDraft}
              locale={calendarLocales[language]}
              disabled={{ after: new Date() }}
              defaultMonth={draft?.from}
            />
            <Separator />
            <div className="flex items-center justify-between gap-2 p-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setPicking(false)}
              >
                {t('history.date.back')}
              </Button>
              <Button
                size="sm"
                disabled={!draft?.from}
                onClick={() =>
                  draft?.from &&
                  choose(rangeParam(draft.from, draft.to ?? draft.from))
                }
              >
                {t('history.date.apply')}
              </Button>
            </div>
          </div>
        ) : (
          <div role="menu" className="flex flex-col">
            {presets.map((preset) => {
              const active =
                (filter.kind === 'all' && preset.value === null) ||
                (filter.kind === 'preset' && filter.preset === preset.value)
              return (
                <button
                  key={preset.label}
                  type="button"
                  role="menuitemradio"
                  aria-checked={active}
                  onClick={() => choose(preset.value)}
                  className="flex h-8 items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-muted focus-visible:bg-muted"
                >
                  <Check className={cn('size-3.5', !active && 'opacity-0')} />
                  {t(preset.label)}
                </button>
              )
            })}
            <Separator className="my-1" />
            <button
              type="button"
              role="menuitem"
              onClick={openCalendar}
              className="flex h-8 items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-muted focus-visible:bg-muted"
            >
              <Check
                className={cn(
                  'size-3.5',
                  filter.kind !== 'range' && 'opacity-0',
                )}
              />
              {t('history.date.custom')}
            </button>
          </div>
        )}
      </PopoverContent>
    </Popover>
  )
}
