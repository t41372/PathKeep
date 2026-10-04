/**
 * Import another browser's history file or profile folder (Browser Direct).
 * Preview first: how many visits are in the file, how many are new and how
 * many the archive already has, the time range and the latest entries. The
 * import adds exactly the new ones; it can be undone from Recent imports.
 *
 * The state machine lives in `use-browser-import.ts`; the window-wide file
 * drop is routed by the page, so this card and the Takeout card never both
 * react to one drop.
 */
import { FolderOpen, History, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { useBackupRunner } from '@/app/backup-runner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import type { TakeoutInspection } from '@/lib/types'
import { canUseNativeFiles, pickBrowserHistory } from './file-drop'
import {
  browserSources,
  newVisits,
  type BrowserImportState,
  type BrowserSource,
} from './use-browser-import'

const baseName = (path: string) =>
  path.split(/[\\/]/).filter(Boolean).pop() ?? path

const detectedFamilies: Record<string, 'chromium' | 'firefox' | 'safari'> = {
  'chromium-history-db': 'chromium',
  'firefox-places-db': 'firefox',
  'safari-history-db': 'safari',
}

function Entry({
  hovering,
  disabled,
  source,
  onSource,
  onPath,
}: {
  hovering: boolean
  disabled: boolean
  source: BrowserSource
  onSource: (source: BrowserSource) => void
  onPath: (path: string) => void
}) {
  const { t } = useI18n()
  const [typed, setTyped] = useState('')
  const native = canUseNativeFiles()

  async function choose(directory: boolean) {
    try {
      const path = await pickBrowserHistory(
        directory
          ? t('backupImport.browser.folderPickerTitle')
          : t('backupImport.browser.pickerTitle'),
        directory,
      )
      if (path) onPath(path)
    } catch (error) {
      toast.error(t('backup.import.pickFailed'), {
        description: describeError(error, 'dialog_open'),
      })
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div
        className={cn(
          'flex flex-col items-center gap-3 rounded-lg border border-dashed px-4 py-5 text-center transition-colors',
          hovering && 'border-brand bg-brand-soft',
        )}
      >
        <History className="size-5 text-muted-foreground" />
        <p className="text-[13px]">
          {hovering
            ? t('backup.import.dropActive')
            : t('backupImport.browser.drop')}
        </p>
        {native ? (
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={disabled}
              onClick={() => void choose(false)}
            >
              {t('backupImport.browser.chooseFile')}
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={disabled}
              onClick={() => void choose(true)}
            >
              <FolderOpen />
              {t('backupImport.browser.chooseFolder')}
            </Button>
          </div>
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
              placeholder={t('backupImport.browser.pathPlaceholder')}
              aria-label={t('backupImport.browser.pathLabel')}
              className="h-8 font-mono text-xs"
            />
            <Button
              size="sm"
              variant="outline"
              type="submit"
              disabled={disabled || !typed.trim()}
            >
              {t('backupImport.browser.inspect')}
            </Button>
          </form>
        )}
      </div>
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span className="text-muted-foreground">
          {t('backupImport.browser.from')}
        </span>
        <Select
          value={source}
          onValueChange={(value) => onSource(value as BrowserSource)}
        >
          <SelectTrigger
            size="sm"
            className="w-48"
            aria-label={t('backupImport.browser.from')}
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {browserSources.map((value) => (
              <SelectItem key={value} value={value}>
                {t(`backupImport.browser.sources.${value}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </div>
  )
}

function Preview({
  path,
  source,
  inspection,
}: {
  path: string
  source: BrowserSource
  inspection: TakeoutInspection
}) {
  const { t } = useI18n()
  const format = useFormat()
  const fresh = newVisits(inspection)
  const family = detectedFamilies[inspection.recognizedFiles[0]?.kind ?? '']
  const browser =
    source !== 'detect'
      ? t(`backupImport.browser.sources.${source}`)
      : family
        ? t(`backupImport.browser.detected.${family}`)
        : null
  const sample = inspection.previewEntries.slice(0, 5)

  return (
    <div className="flex flex-col gap-3">
      <div
        className="flex flex-col gap-1 rounded-lg bg-muted p-3 text-[13px]"
        data-testid="browser-import-preview"
      >
        <span
          className="truncate font-mono text-xs text-muted-foreground"
          title={inspection.recognizedFiles[0]?.path ?? path}
        >
          {baseName(inspection.recognizedFiles[0]?.path ?? path)}
          {browser && ` · ${browser}`}
        </span>
        <span className="text-base font-semibold tabular">
          {t('backupImport.browser.newVisits', { count: fresh })}
        </span>
        <span className="text-muted-foreground tabular">
          {t('backupImport.browser.already', {
            count: inspection.duplicateItems,
          })}
        </span>
        <span className="text-muted-foreground tabular">
          {t('backupImport.browser.inFile', {
            count: inspection.candidateItems,
          })}
          {inspection.previewRangeStart &&
            inspection.previewRangeEnd &&
            ` · ${t('backupImport.browser.range', {
              start: format.date(inspection.previewRangeStart),
              end: format.date(inspection.previewRangeEnd),
            })}`}
        </span>
      </div>
      {sample.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-xs text-muted-foreground">
            {t('backupImport.browser.sample')}
          </span>
          <ul className="flex flex-col text-[13px]">
            {sample.map((entry) => (
              <li
                key={`${entry.sourceVisitId}-${entry.visitedAt}`}
                className="flex items-baseline gap-2 py-0.5"
              >
                <span className="min-w-0 flex-1 truncate" title={entry.url}>
                  {entry.title ||
                    entry.url ||
                    t('backupImport.browser.untitled')}
                </span>
                {entry.status === 'duplicate' && (
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {t('backupImport.browser.alreadySaved')}
                  </span>
                )}
                <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
                  {format.dayAndTime(entry.visitedAt)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <p className="text-xs text-muted-foreground">
        {family !== 'chromium' && `${t('backupImport.browser.historyOnly')} `}
        {t('backupImport.browser.copyOnly')}
      </p>
    </div>
  )
}

function Importing({
  progress,
}: {
  progress: Extract<BrowserImportState, { step: 'importing' }>['progress']
}) {
  const { t } = useI18n()
  const format = useFormat()
  const percent = progress?.progressPercent
  return (
    <div className="flex flex-col gap-2.5 rounded-lg bg-muted p-3 text-[13px]">
      <div className="flex items-baseline justify-between gap-3">
        <span className="flex items-center gap-2">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {t('backupImport.browser.running')}
        </span>
        {progress?.processedRecords != null && (
          <span className="font-mono text-xs text-muted-foreground tabular">
            {progress.totalRecords
              ? t('backupImport.browser.processed', {
                  done: format.number(progress.processedRecords),
                  total: format.number(progress.totalRecords),
                })
              : format.number(progress.processedRecords)}
          </span>
        )}
      </div>
      <div
        role="progressbar"
        aria-label={t('backupImport.browser.running')}
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

export function BrowserImportCard({
  state,
  hovering,
  onInspect,
  onConfirm,
  onReset,
}: {
  state: BrowserImportState
  hovering: boolean
  onInspect: (path: string, source: BrowserSource) => void
  onConfirm: () => void
  onReset: () => void
}) {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const [source, setSource] = useState<BrowserSource>('detect')

  return (
    <SectionCard
      title={t('backupImport.browser.title')}
      subtitle={t('backupImport.browser.subtitle')}
    >
      {state.step === 'idle' && (
        <>
          <Entry
            hovering={hovering}
            disabled={backup.running}
            source={source}
            onSource={setSource}
            onPath={(path) => onInspect(path, source)}
          />
          {state.error && (
            <p
              role="alert"
              className="text-[13px] break-words text-destructive"
            >
              <strong className="font-medium">
                {t('backupImport.browser.inspectFailed')}
              </strong>{' '}
              {state.error}
            </p>
          )}
        </>
      )}
      {state.step === 'inspecting' && (
        <p className="flex items-center gap-2 text-[13px] text-muted-foreground">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {t('backupImport.browser.inspecting', { name: baseName(state.path) })}
        </p>
      )}
      {state.step === 'preview' && (
        <>
          <Preview
            path={state.path}
            source={state.source}
            inspection={state.inspection}
          />
          {newVisits(state.inspection) === 0 && (
            <p className="text-[13px] text-muted-foreground">
              {t('backupImport.browser.nothingNew')}
            </p>
          )}
          {backup.running && (
            <p className="text-[13px] text-muted-foreground">
              {t('backupImport.browser.busy')}
            </p>
          )}
          <div className="flex gap-2">
            {newVisits(state.inspection) > 0 && (
              <Button size="sm" disabled={backup.running} onClick={onConfirm}>
                {t('backupImport.browser.confirm', {
                  count: newVisits(state.inspection),
                })}
              </Button>
            )}
            <Button size="sm" variant="ghost" onClick={onReset}>
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
              {t('backupImport.browser.done', {
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
            <Button size="sm" variant="outline" onClick={onReset}>
              {t('backupImport.browser.another')}
            </Button>
          </div>
        </>
      )}
    </SectionCard>
  )
}
