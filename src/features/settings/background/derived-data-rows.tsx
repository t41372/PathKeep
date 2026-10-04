/**
 * Search and Insights data: what PathKeep worked out from the history, per
 * part (version, freshness, last build), the extra page details plugins
 * add, and "clear" with a real preview of what goes.
 *
 * Clear (see `clear-derived-dialog`) is refused while a rebuild is running,
 * since the clear also drops rebuild jobs.
 *
 * Not responsible for the queues (see `queue-rows`).
 */
import { useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { DeterministicModuleRuntimeStatus } from '@/lib/types'
import { ClearDerivedDialog } from './clear-derived-dialog'
import { useIntelligenceRuntime } from './use-background-work'

const moduleIds = [
  'visit-derived-facts',
  'daily-rollups',
  'sessions',
  'search-trails',
  'refind-pages',
  'activity-mix',
  'search-effectiveness',
  'domain-deep-dive',
] as const
const moduleStatuses = [
  'ready',
  'stale',
  'idle',
  'disabled',
  'running',
  'failed',
] as const
const staleCodes = [
  'module-version-changed',
  'missing-build-timestamp',
  'archive-data-changed',
  'visibility-or-rollback-changed',
] as const
const pluginIds = ['title-normalization', 'readable-content-refetch'] as const

function oneOf<T extends string>(list: readonly T[], value?: string | null) {
  return (list as readonly (string | null | undefined)[]).includes(value)
    ? (value as T)
    : null
}

const statusStyle: Record<(typeof moduleStatuses)[number], string> = {
  ready: 'bg-green/12 text-green',
  stale: 'bg-brand-soft text-brand',
  idle: 'bg-muted text-muted-foreground',
  disabled: 'bg-muted text-muted-foreground',
  running: 'bg-brand-soft text-brand',
  failed: 'bg-destructive/10 text-destructive',
}

export function InsightsDataRow() {
  const { t } = useI18n()
  const paused = useSnapshot().config.ai.jobQueuePaused
  const runtime = useIntelligenceRuntime(paused)
  const [open, setOpen] = useState(false)
  const busy = (runtime.data?.queue.running ?? 0) > 0

  return (
    <SettingRow
      title={t('settingsBackground.data.title')}
      description={
        busy
          ? t('settingsBackground.data.clearBusy')
          : t('settingsBackground.data.description')
      }
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={!runtime.data || busy}
          onClick={() => setOpen(true)}
        >
          {t('settingsBackground.data.clear')}
        </Button>
      }
    >
      {runtime.isPending ? (
        <Skeleton className="h-24 w-full" />
      ) : runtime.data ? (
        <ul
          aria-label={t('settingsBackground.data.title')}
          className="flex flex-col border-t pt-1"
        >
          {runtime.data.modules.map((module) => (
            <ModuleItem key={module.moduleId} module={module} />
          ))}
        </ul>
      ) : null}
      {open && <ClearDerivedDialog onClose={() => setOpen(false)} />}
    </SettingRow>
  )
}

function ModuleItem({ module }: { module: DeterministicModuleRuntimeStatus }) {
  const { t } = useI18n()
  const format = useFormat()
  const id = oneOf(moduleIds, module.moduleId)
  const status = oneOf(moduleStatuses, module.status)
  const stale = oneOf(staleCodes, module.staleReasonCode)
  const name = id ? t(`settingsBackground.data.module.${id}`) : module.moduleId
  return (
    <li
      className="flex items-center gap-3 border-b py-2 last:border-b-0"
      data-module={module.moduleId}
      data-status={module.status}
    >
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-[13px] font-medium">{name}</span>
        <span className="text-xs text-muted-foreground">
          {t('settingsBackground.data.version', { version: module.version })}
          {module.lastBuiltAt &&
            ` · ${t('settingsBackground.data.builtAt', {
              when: format.relative(module.lastBuiltAt),
            })}`}
          {stale && ` · ${t(`settingsBackground.data.stale.${stale}`)}`}
        </span>
      </div>
      <Badge
        variant="secondary"
        className={cn(
          'min-w-[76px] justify-center rounded-md border-0 font-normal',
          status ? statusStyle[status] : 'bg-muted text-muted-foreground',
        )}
      >
        {status ? t(`settingsBackground.data.status.${status}`) : module.status}
      </Badge>
    </li>
  )
}

export function PageDetailsRow() {
  const { t } = useI18n()
  const format = useFormat()
  const paused = useSnapshot().config.ai.jobQueuePaused
  const runtime = useIntelligenceRuntime(paused)
  return (
    <SettingRow
      title={t('settingsBackground.data.plugins.title')}
      description={t('settingsBackground.data.plugins.description')}
    >
      {runtime.isPending ? (
        <Skeleton className="h-12 w-full" />
      ) : runtime.data ? (
        <ul className="flex flex-col border-t pt-1">
          {runtime.data.plugins.map((plugin) => {
            const id = oneOf(pluginIds, plugin.pluginId)
            return (
              <li
                key={plugin.pluginId}
                className="flex flex-col gap-0.5 border-b py-2 text-[13px] last:border-b-0"
              >
                <div className="flex items-center justify-between gap-3">
                  <span className="font-medium">
                    {id
                      ? t(`settingsBackground.data.plugins.${id}`)
                      : plugin.pluginId}
                  </span>
                  <span className="text-xs text-muted-foreground tabular-nums">
                    {plugin.enabled
                      ? t('settingsBackground.data.plugins.stored', {
                          count: plugin.storedRecords,
                        })
                      : t('settingsBackground.data.plugins.off')}
                  </span>
                </div>
                <span className="text-xs text-muted-foreground">
                  {plugin.lastCompletedAt
                    ? t('settingsBackground.data.plugins.lastRun', {
                        when: format.relative(plugin.lastCompletedAt),
                      })
                    : t('settingsBackground.data.plugins.neverRun')}
                </span>
                {plugin.lastError && (
                  <span className="text-xs break-words text-destructive">
                    {t('settingsBackground.data.plugins.lastError', {
                      message: plugin.lastError,
                    })}
                  </span>
                )}
              </li>
            )
          })}
        </ul>
      ) : null}
    </SettingRow>
  )
}
