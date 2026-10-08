/**
 * Progress and a Stop button for a long whole-archive operation (export,
 * import, encryption change, restore) while its command runs. The backend
 * reports progress through its own cheap command, so this polls that instead
 * of waiting on the operation.
 */
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { maintenanceClient } from '@/lib/backend-client/maintenance'
import { useFormat, useI18n } from '@/lib/i18n'

const POLL_MS = 500

const phases = [
  'exporting',
  'importing',
  'converting',
  'rewrapping',
  'restoring',
  'pruning',
  'copying',
  'snapshotting',
  'verifying',
  'indexing',
  'projecting',
  'publishing',
  'merging',
] as const
type Phase = (typeof phases)[number]
const isPhase = (phase: string): phase is Phase =>
  (phases as readonly string[]).includes(phase)

/** `onStop` lets the caller tell a stop it asked for from a failure. */
export function MaintenanceProgress({ onStop }: { onStop: () => void }) {
  const { t } = useI18n()
  const format = useFormat()
  const [stopping, setStopping] = useState(false)
  const progress = useQuery({
    queryKey: ['maintenance-progress'],
    queryFn: maintenanceClient.progress,
    refetchInterval: POLL_MS,
    gcTime: 0,
  })
  const current = progress.data ?? null
  const percent =
    current && current.total > 0
      ? Math.min(100, (current.processed / current.total) * 100)
      : undefined

  return (
    <div role="status" className="flex flex-col gap-1.5">
      <Progress
        value={percent}
        className="h-1.5"
        aria-label={t('maintenance.progressLabel')}
      />
      <div className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
        <span className="min-w-0 truncate tabular">
          {current &&
            isPhase(current.phase) &&
            t(`maintenance.phases.${current.phase}`)}
          {current?.table && ` · ${current.table}`}
          {current && current.total > 0
            ? ` · ${t('maintenance.progress', {
                done: format.number(current.processed),
                total: format.number(current.total),
              })}`
            : ''}
        </span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={stopping}
          onClick={() => {
            setStopping(true)
            onStop()
            void maintenanceClient.cancel()
          }}
        >
          {stopping ? t('maintenance.stopping') : t('maintenance.stop')}
        </Button>
      </div>
    </div>
  )
}
