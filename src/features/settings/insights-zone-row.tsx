/**
 * Settings → General → Insights time zone: the zone Insights put days and
 * hours in, chosen from every zone this window knows, searched by city, zone
 * name or IANA name. Choosing another one builds Insights again; the row says
 * so until the rebuild is done, and shows the notice when the computer has
 * moved to another zone (`insights/zone-notice.tsx`).
 */
import { Check, ChevronsUpDown } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { useI18n } from '@/lib/i18n'
import {
  computerTimeZone,
  hasZoneLabel,
  knownTimeZones,
  zoneLabel,
} from '@/lib/time-zone'
import {
  useChooseInsightsZone,
  useInsightsZone,
  zoneText,
} from '@/features/insights/zone'
import { ZoneNotice } from '@/features/insights/zone-notice'

/**
 * Labels every zone a few at a time after the row appears, so the list has
 * names and offsets when it opens without one long task on the main thread
 * (about 400 zones, two `Intl` formatters each).
 */
function useZoneLabels(zones: string[], locale: string) {
  const [, setLabelled] = useState(0)
  const ready = zones.every((zone) => hasZoneLabel(zone, locale))
  useEffect(() => {
    if (ready) return
    let index = 0
    let timer = 0
    const step = () => {
      const until = performance.now() + 6
      while (index < zones.length && performance.now() < until) {
        zoneLabel(zones[index], locale)
        index += 1
      }
      if (index < zones.length) timer = window.setTimeout(step, 0)
      else setLabelled((count) => count + 1)
    }
    timer = window.setTimeout(step, 0)
    return () => window.clearTimeout(timer)
  }, [zones, locale, ready])
  return ready
}

export function InsightsZoneRow() {
  const { t, locale } = useI18n()
  const zone = useInsightsZone()
  const { choose, saving } = useChooseInsightsZone()
  const [open, setOpen] = useState(false)
  const zones = useMemo(
    () => knownTimeZones([zone.next, zone.shown, computerTimeZone()]).sort(),
    [zone.next, zone.shown],
  )
  const labelled = useZoneLabels(zones, locale)
  const current = zoneLabel(zone.next, locale)
  const rebuilding = zone.shown !== zone.next

  return (
    <SettingRow
      title={t('settings.general.insightsZone.title')}
      htmlFor="settings-insights-zone"
      description={
        <>
          {t('settings.general.insightsZone.description')}
          {rebuilding && (
            <span className="mt-1 block text-foreground">
              {t('settings.general.insightsZone.rebuilding', {
                zone: zoneText(zone.next, locale, t, [zone.shown]),
                old: zoneText(zone.shown, locale, t, [zone.next]),
              })}
            </span>
          )}
        </>
      }
      control={
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button
              id="settings-insights-zone"
              variant="outline"
              size="sm"
              role="combobox"
              aria-expanded={open}
              disabled={saving}
              className="min-w-[160px] justify-between bg-card text-[13px] font-normal"
            >
              <span className="truncate">
                {current.place}
                <span className="ml-1.5 text-muted-foreground tabular">
                  {current.offset}
                </span>
              </span>
              <ChevronsUpDown className="size-3.5 opacity-50" aria-hidden />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-[320px] p-0">
            <Command>
              <CommandInput
                placeholder={t('settings.general.insightsZone.search')}
              />
              <CommandList>
                <CommandEmpty>
                  {t('settings.general.insightsZone.none')}
                </CommandEmpty>
                {zones.map((id) => {
                  const label = labelled ? zoneLabel(id, locale) : null
                  const place = id.split('/').at(-1)?.replaceAll('_', ' ')
                  return (
                    <CommandItem
                      key={id}
                      value={id}
                      aria-label={
                        label
                          ? [place, label.name || id, label.offset].join(', ')
                          : place
                      }
                      keywords={[
                        id.replaceAll('_', ' '),
                        ...(label ? [label.name, label.offset] : []),
                      ]}
                      onSelect={() => {
                        setOpen(false)
                        if (id !== zone.next) void choose(id)
                      }}
                      className="text-[13px]"
                    >
                      <Check
                        className={
                          id === zone.next ? 'size-3.5' : 'size-3.5 opacity-0'
                        }
                        aria-hidden
                      />
                      <span className="flex min-w-0 flex-1 flex-col">
                        <span className="truncate">{place}</span>
                        <span className="truncate text-xs text-muted-foreground">
                          {label?.name || id}
                        </span>
                      </span>
                      <span className="shrink-0 text-xs text-muted-foreground tabular">
                        {label?.offset}
                      </span>
                    </CommandItem>
                  )
                })}
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      }
    >
      <ZoneNotice />
    </SettingRow>
  )
}
