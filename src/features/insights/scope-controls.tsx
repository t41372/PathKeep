/**
 * The scope controls in the header of Insights and its drill-ins: the time
 * range (all time, the presets, or a custom first and last day) and, when the
 * archive has more than one browser profile with visits, which profile.
 *
 * Responsible for: showing and changing the scope in the URL.
 * Not responsible for: what a scope covers (`range.ts`).
 */
import { CalendarRange } from 'lucide-react'
import { enUS, zhCN, zhTW } from 'date-fns/locale'
import { useState } from 'react'
import type { DateRange as PickedDays } from 'react-day-picker'
import { Button } from '@/components/ui/button'
import { Calendar } from '@/components/ui/calendar'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { localDateKey } from '@/lib/backend-client/insights'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSourceStats } from '@/features/home/queries'
import { isPreset, presets, useScope, type Scope } from './range'

const calendarLocales = { en: enUS, 'zh-CN': zhCN, 'zh-TW': zhTW }
const ALL_PROFILES = 'all'
const parse = (key: string) => new Date(`${key}T00:00:00`)

export function ScopeControls() {
  return (
    <div className="flex flex-wrap items-center justify-end gap-2">
      <ProfileSelect />
      <RangeToggle />
    </div>
  )
}

function RangeToggle() {
  const { t, language } = useI18n()
  const format = useFormat()
  const [scope, setScope] = useScope()
  const [picking, setPicking] = useState(false)
  const [draft, setDraft] = useState<PickedDays | undefined>()

  const startPicking = () => {
    setDraft(
      scope.range === 'custom'
        ? { from: parse(scope.dates.start), to: parse(scope.dates.end) }
        : undefined,
    )
    setPicking(true)
  }
  const customLabel = (current: Scope) => {
    const options = { month: 'short', day: 'numeric', year: 'numeric' } as const
    const start = format.date(parse(current.dates.start), options)
    const end = format.date(parse(current.dates.end), options)
    return start === end ? start : `${start} – ${end}`
  }

  return (
    <Popover open={picking} onOpenChange={setPicking}>
      <PopoverAnchor asChild>
        <ToggleGroup
          type="single"
          value={scope.range}
          aria-label={t('insights.range.label')}
          onValueChange={(next) => {
            if (isPreset(next)) setScope({ range: next })
          }}
          className="rounded-[9px] bg-muted p-[3px]"
        >
          {presets.map((id) => (
            <ToggleGroupItem
              key={id}
              value={id}
              className="h-[30px] rounded-[7px] px-3 text-[13px] data-[state=on]:bg-card data-[state=on]:shadow-card"
            >
              {t(`insights.range.${id}`)}
            </ToggleGroupItem>
          ))}
          <ToggleGroupItem
            value="custom"
            aria-label={
              scope.range === 'custom'
                ? t('insights.range.customLabel', { range: customLabel(scope) })
                : t('insights.range.custom')
            }
            // A custom range is chosen in the calendar, also when it is
            // already the current one.
            onClick={(event) => {
              event.preventDefault()
              startPicking()
            }}
            className="h-[30px] gap-1.5 rounded-[7px] px-3 text-[13px] data-[state=on]:bg-card data-[state=on]:shadow-card"
          >
            <CalendarRange className="size-3.5" aria-hidden />
            {scope.range === 'custom'
              ? customLabel(scope)
              : t('insights.range.custom')}
          </ToggleGroupItem>
        </ToggleGroup>
      </PopoverAnchor>
      <PopoverContent align="end" className="w-auto p-1">
        <p className="px-2 pt-1.5 text-[13px] text-muted-foreground">
          {t('insights.range.pick')}
        </p>
        <Calendar
          mode="range"
          selected={draft}
          onSelect={setDraft}
          locale={calendarLocales[language]}
          disabled={{ after: new Date() }}
          defaultMonth={draft?.from}
          captionLayout="dropdown"
        />
        <Separator />
        <div className="flex items-center justify-end gap-2 p-2">
          <Button variant="ghost" size="sm" onClick={() => setPicking(false)}>
            {t('insights.range.cancel')}
          </Button>
          <Button
            size="sm"
            disabled={!draft?.from}
            onClick={() => {
              if (!draft?.from) return
              setScope({
                range: 'custom',
                dates: {
                  start: localDateKey(draft.from),
                  end: localDateKey(draft.to ?? draft.from),
                },
              })
              setPicking(false)
            }}
          >
            {t('insights.range.apply')}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}

/** Hidden until there are two profiles with visits to choose between. */
function ProfileSelect() {
  const { t } = useI18n()
  const [scope, setScope] = useScope()
  const sources = useSourceStats()
  const profiles = (sources.data ?? [])
    .filter((source) => source.visitCount > 0)
    .sort((a, b) => b.visitCount - a.visitCount)
  if (profiles.length < 2 && !scope.profileId) return null
  return (
    <Select
      value={scope.profileId ?? ALL_PROFILES}
      onValueChange={(value) =>
        setScope({ profileId: value === ALL_PROFILES ? null : value })
      }
    >
      <SelectTrigger
        size="sm"
        aria-label={t('insights.profile.label')}
        className="h-9 max-w-[240px] bg-card text-[13px]"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent align="end">
        <SelectItem value={ALL_PROFILES}>
          {t('insights.profile.all')}
        </SelectItem>
        {profiles.map((source) => (
          <SelectItem key={source.profileId} value={source.profileId}>
            {source.browserName} · {source.profileName}
          </SelectItem>
        ))}
        {scope.profileId &&
          !profiles.some((source) => source.profileId === scope.profileId) && (
            <SelectItem value={scope.profileId}>{scope.profileId}</SelectItem>
          )}
      </SelectContent>
    </Select>
  )
}
