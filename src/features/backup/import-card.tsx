/**
 * Import: bring in a Google Takeout export. Follows preview → confirm →
 * import, so nothing is written until the user has seen what was found.
 * Earlier imports are listed underneath with undo.
 */
import { FileArchive, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { useBackupRunner } from '@/app/backup-runner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import type { TakeoutInspection } from '@/lib/types'
import {
  canUseNativeFiles,
  pickTakeoutFile,
  useWindowFileDrop,
} from './file-drop'
import { ImportBatches } from './import-batches'
import { useTakeoutImport, type ImportState } from './use-takeout-import'

const baseName = (path: string) =>
  path.split(/[\\/]/).filter(Boolean).pop() ?? path

function DropZone({
  hovering,
  disabled,
  onPath,
}: {
  hovering: boolean
  disabled: boolean
  onPath: (path: string) => void
}) {
  const { t } = useI18n()
  const [typed, setTyped] = useState('')
  const native = canUseNativeFiles()

  async function choose() {
    try {
      const path = await pickTakeoutFile(t('backup.import.pickerTitle'))
      if (path) onPath(path)
    } catch (error) {
      toast.error(t('backup.import.pickFailed'), {
        description: describeError(error, 'dialog_open'),
      })
    }
  }

  return (
    <div
      className={cn(
        'flex flex-col items-center gap-3 rounded-lg border border-dashed px-4 py-5 text-center transition-colors',
        hovering && 'border-brand bg-brand-soft',
      )}
    >
      <FileArchive className="size-5 text-muted-foreground" />
      <p className="text-[13px]">
        {hovering ? t('backup.import.dropActive') : t('backup.import.drop')}
      </p>
      {native ? (
        <Button
          size="sm"
          variant="outline"
          disabled={disabled}
          onClick={() => void choose()}
        >
          {t('backup.import.choose')}
        </Button>
      ) : (
        <form
          className="flex w-full gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            onPath(typed)
          }}
        >
          <Input
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            placeholder={t('backup.import.pathPlaceholder')}
            aria-label={t('backup.import.pathLabel')}
            className="h-8 font-mono text-xs"
          />
          <Button
            size="sm"
            variant="outline"
            type="submit"
            disabled={disabled || !typed.trim()}
          >
            {t('backup.import.inspect')}
          </Button>
        </form>
      )}
    </div>
  )
}

function Found({
  inspection,
  path,
}: {
  inspection: TakeoutInspection
  path: string
}) {
  const { t } = useI18n()
  const format = useFormat()
  const empty = inspection.candidateItems === 0
  return (
    <div className="flex flex-col gap-1 rounded-lg bg-muted p-3 text-[13px]">
      <span
        className="truncate font-mono text-xs text-muted-foreground"
        title={path}
      >
        {baseName(path)}
      </span>
      <span className="text-base font-semibold tabular">
        {empty
          ? t('backup.import.nothing')
          : t('backup.import.found', { count: inspection.candidateItems })}
      </span>
      {!empty && inspection.previewRangeStart && inspection.previewRangeEnd && (
        <span className="text-muted-foreground">
          {t('backup.import.range', {
            start: format.date(inspection.previewRangeStart),
            end: format.date(inspection.previewRangeEnd),
          })}
        </span>
      )}
      {inspection.quarantinedFiles.length > 0 && (
        <span className="text-muted-foreground">
          {t('backup.import.setAside', {
            count: inspection.quarantinedFiles.length,
          })}
        </span>
      )}
      {!empty && (
        <span className="text-muted-foreground">
          {t('backup.import.dedupe')}
        </span>
      )}
    </div>
  )
}

function Importing({
  progress,
}: {
  progress: Extract<ImportState, { step: 'importing' }>['progress']
}) {
  const { t } = useI18n()
  const format = useFormat()
  const percent = progress?.progressPercent
  return (
    <div className="flex flex-col gap-2.5 rounded-lg bg-muted p-3 text-[13px]">
      <div className="flex items-baseline justify-between gap-3">
        <span className="flex items-center gap-2">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {t('backup.import.running')}
        </span>
        {progress?.processedRecords != null && progress.totalRecords ? (
          <span className="font-mono text-xs text-muted-foreground tabular">
            {format.number(progress.processedRecords)} /{' '}
            {format.number(progress.totalRecords)}
          </span>
        ) : null}
      </div>
      <div
        role="progressbar"
        aria-label={t('backup.import.running')}
        aria-valuenow={percent == null ? undefined : Math.round(percent)}
        className="h-1 overflow-hidden rounded-full bg-background/60"
      >
        <div
          className={cn(
            'h-full rounded-full bg-brand',
            percent == null
              ? 'w-1/3 animate-indeterminate'
              : 'transition-[width]',
          )}
          style={
            percent == null
              ? undefined
              : { width: `${Math.min(100, Math.max(2, percent))}%` }
          }
        />
      </div>
    </div>
  )
}

export function ImportCard() {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const { state, inspect, confirm, reset } = useTakeoutImport()
  const hovering = useWindowFileDrop(
    (path) => void inspect(path),
    state.step === 'idle' && !backup.running,
  )

  return (
    <SectionCard
      title={t('backup.import.title')}
      subtitle={t('backup.import.subtitle')}
    >
      {state.step === 'idle' && (
        <>
          <DropZone
            hovering={hovering}
            disabled={backup.running}
            onPath={(path) => void inspect(path)}
          />
          {state.error && (
            <p
              role="alert"
              className="text-[13px] break-words text-destructive"
            >
              <strong className="font-medium">
                {t('backup.import.inspectFailed')}
              </strong>{' '}
              {state.error}
            </p>
          )}
        </>
      )}
      {state.step === 'inspecting' && (
        <p className="flex items-center gap-2 text-[13px] text-muted-foreground">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {t('backup.import.inspecting', { name: baseName(state.path) })}
        </p>
      )}
      {state.step === 'preview' && (
        <>
          <Found inspection={state.inspection} path={state.path} />
          <div className="flex gap-2">
            {state.inspection.candidateItems > 0 && (
              <Button
                size="sm"
                disabled={backup.running}
                onClick={() => void confirm()}
              >
                {t('backup.import.confirm')}
              </Button>
            )}
            <Button size="sm" variant="ghost" onClick={reset}>
              {t('common.cancel')}
            </Button>
          </div>
        </>
      )}
      {state.step === 'importing' && <Importing progress={state.progress} />}
      {state.step === 'done' && (
        <>
          <div className="flex flex-col gap-1 rounded-lg bg-muted p-3 text-[13px]">
            <span className="text-base font-semibold tabular">
              {t('backup.import.imported', {
                count: state.result.importedItems,
              })}
            </span>
            <span className="text-muted-foreground">
              {t('backup.import.duplicates', {
                count: state.result.duplicateItems,
              })}
            </span>
          </div>
          <div>
            <Button size="sm" variant="outline" onClick={reset}>
              {t('backup.import.another')}
            </Button>
          </div>
        </>
      )}
      <ImportBatches />
    </SectionCard>
  )
}
