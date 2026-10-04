/**
 * Settings → Storage → Export history: one export at a time, how far it has
 * got, and a way to stop it.
 *
 * The backend writes the file; this row only polls its progress by id. The id
 * travels in the mutation's variables, so leaving Settings and coming back
 * finds the running export instead of offering to start a second one.
 */
import {
  useMutation,
  useMutationState,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import type { ExportFormat, ExportProgress } from '@/lib/types'
import { reveal, revealLabelKey } from './reveal'
import { RowSelect } from './row-select'

const formats: ExportFormat[] = ['html', 'markdown', 'text', 'jsonl']
const exportMutationKey = ['export-history'] as const
const POLL_MS = 500
/** Before this the rate is mostly file setup and a cold cache, so a guess would be wild. */
const ESTIMATE_AFTER_MS = 2_000

interface ExportRun {
  exportId: string
  format: ExportFormat
}

export function ExportRow() {
  const { t } = useI18n()
  const client = useQueryClient()
  const [kind, setKind] = useState<ExportFormat>('html')
  const start = useMutation({
    mutationKey: exportMutationKey,
    mutationFn: ({ exportId, format }: ExportRun) =>
      archiveClient.exportHistory({ exportId, query: {}, format }),
    onSuccess: (result) => {
      void client.invalidateQueries({ queryKey: queryKeys.dashboard })
      toast.success(t('settingsStorage.export.done', { count: result.count }), {
        description: result.path,
        action: {
          label: t(revealLabelKey()),
          onClick: () =>
            void reveal(result.path, t('settingsStorage.location.failed')),
        },
      })
    },
    onError: (error) => {
      if (commandErrorCode(error) === 'export-cancelled') {
        toast(t('settingsStorage.export.cancelled'))
        return
      }
      toast.error(t('settingsStorage.export.failed'), {
        description: describeError(error, 'export_history'),
      })
    },
  })
  // Also finds an export this row started before it was last unmounted.
  const running = useMutationState({
    filters: { mutationKey: exportMutationKey, status: 'pending' },
    select: (mutation) => mutation.state.variables as ExportRun,
  }).at(-1)
  const progress = useQuery({
    queryKey: queryKeys.exportProgress(running?.exportId ?? ''),
    queryFn: () => archiveClient.getExportProgress(running!.exportId),
    enabled: running != null,
    refetchInterval: POLL_MS,
  })
  const stop = useMutation({
    mutationFn: (exportId: string) => archiveClient.cancelExport(exportId),
  })
  const stopping =
    running != null &&
    stop.variables === running.exportId &&
    (stop.isPending || stop.data === true)
  const finishing = progress.data?.state === 'finishing'

  return (
    <SettingRow
      title={t('settingsStorage.export.title')}
      description={t('settingsStorage.export.description')}
      control={
        <>
          <RowSelect
            value={running?.format ?? kind}
            label={t('settingsStorage.export.format')}
            disabled={running != null}
            onChange={(value) => setKind(value as ExportFormat)}
            options={formats.map((value) => ({
              value,
              label: t(`settingsStorage.export.formats.${value}`),
            }))}
          />
          {running ? (
            <Button
              size="sm"
              variant="outline"
              disabled={stopping || finishing}
              onClick={() => stop.mutate(running.exportId)}
            >
              {stopping && <Spinner />}
              {stopping
                ? t('settingsStorage.export.stopping')
                : t('settingsStorage.export.stop')}
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                start.mutate({ exportId: crypto.randomUUID(), format: kind })
              }
            >
              {t('settingsStorage.export.action')}
            </Button>
          )}
        </>
      }
    >
      {running && (
        <ProgressReadout
          progress={progress.data ?? null}
          sampledAt={progress.dataUpdatedAt}
        />
      )}
    </SettingRow>
  )
}

/**
 * The bar and the line under it. The total is the archive's cached visit
 * count, which is what an unfiltered export writes, so it is shown as "about".
 */
function ProgressReadout({
  progress,
  sampledAt,
}: {
  progress: ExportProgress | null
  sampledAt: number
}) {
  const { t } = useI18n()
  const format = useFormat()
  const rows = progress?.rowsWritten ?? 0
  const total = progress?.totalRows ?? null
  const finishing = progress?.state === 'finishing'

  let value: number | null = null
  if (finishing) value = 100
  else if (total) value = Math.min(99, (rows / total) * 100)

  const size = format.bytes(progress?.bytesWritten ?? 0)
  const parts = [
    total
      ? t('settingsStorage.export.progress', {
          written: format.number(rows),
          total: format.number(total),
          size,
        })
      : t('settingsStorage.export.progressNoTotal', {
          written: format.number(rows),
          size,
        }),
  ]
  if (finishing) {
    parts.push(t('settingsStorage.export.finishing'))
  } else if (progress && total && rows > 0 && rows < total) {
    const elapsed = sampledAt - Date.parse(progress.startedAt)
    if (elapsed >= ESTIMATE_AFTER_MS) {
      const seconds = ((total - rows) * elapsed) / rows / 1000
      parts.push(
        seconds < 60
          ? t('settingsStorage.export.leftUnderMinute')
          : t('settingsStorage.export.left', {
              count: Math.ceil(seconds / 60),
            }),
      )
    }
  }

  return (
    <div className="flex flex-col gap-1.5">
      <Progress
        value={value}
        aria-label={t('settingsStorage.export.progressLabel')}
      />
      <p className="text-[13px] text-muted-foreground tabular-nums">
        {parts.join(' · ')}
      </p>
    </div>
  )
}
