/**
 * Local semantic search: the switch, the one-time model download, and the
 * index build, each with live progress. All behavior comes from
 * `useSemanticIndex`; this file only renders it.
 */
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'
import {
  STATIC_MODEL_APPROX_BYTES,
  type useSemanticIndex,
} from '@/features/ai-setup/use-semantic-index'
import { useFormat, useI18n } from '@/lib/i18n'

export type SemanticIndex = ReturnType<typeof useSemanticIndex>

export function SemanticRow({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const format = useFormat()
  // The exact size is only known once the files are on disk; 0 means not yet.
  const size =
    index.status.staticEmbedding?.modelSizeBytes || STATIC_MODEL_APPROX_BYTES
  const on = index.enabled || index.setupRunning

  return (
    <SettingRow
      title={t('settingsAi.semantic.title')}
      description={t('settingsAi.semantic.description', {
        size: format.bytes(size),
      })}
      htmlFor="settings-semantic"
      control={
        <Switch
          id="settings-semantic"
          checked={on}
          disabled={index.saving}
          onCheckedChange={(next) =>
            void (next ? index.enable() : index.disable())
          }
        />
      }
    >
      <SemanticStatus index={index} />
    </SettingRow>
  )
}

function SemanticStatus({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const format = useFormat()
  const { download, status } = index

  if (download.phase === 'downloading') {
    const ratio =
      download.totalBytes > 0
        ? download.downloadedBytes / download.totalBytes
        : 0
    return (
      <ProgressLine
        label={t('settingsAi.semantic.downloading')}
        detail={
          download.totalBytes > 0
            ? t('settingsAi.semantic.downloadProgress', {
                done: format.bytes(download.downloadedBytes),
                total: format.bytes(download.totalBytes),
              })
            : undefined
        }
        value={ratio}
        action={
          <Button
            size="xs"
            variant="ghost"
            onClick={() => void index.cancelDownload()}
          >
            {t('settingsAi.semantic.cancel')}
          </Button>
        }
      />
    )
  }

  if (index.error) {
    return (
      <p
        role="alert"
        className="text-[13px] [overflow-wrap:anywhere] text-destructive"
      >
        {t('settingsAi.semantic.failed', { message: index.error })}
      </p>
    )
  }

  if (!index.enabled) {
    return index.modelDownloaded ? (
      <p className="text-[13px] text-muted-foreground">
        {t('settingsAi.semantic.offNote')}
      </p>
    ) : null
  }

  if (download.phase === 'failed') {
    return (
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span className="text-destructive">
          {t('settingsAi.semantic.downloadFailed', {
            message: download.error ?? '',
          })}
        </span>
        <Button size="xs" variant="outline" onClick={() => void index.enable()}>
          {t('settingsAi.semantic.retry')}
        </Button>
      </div>
    )
  }

  if (!index.modelDownloaded && !index.setupRunning) {
    return (
      <div className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
        {t('settingsAi.semantic.modelMissing')}
        <Button size="xs" variant="outline" onClick={() => void index.enable()}>
          {t('settingsAi.semantic.download')}
        </Button>
      </div>
    )
  }

  if (index.building || index.setupRunning) {
    return index.progress ? (
      <ProgressLine
        label={t('settingsAi.semantic.indexing')}
        detail={t('settingsAi.semantic.indexProgress', {
          done: format.number(index.progress.embedded),
          total: format.number(index.progress.target),
        })}
        value={index.progress.embedded / index.progress.target}
      />
    ) : (
      <ProgressLine label={t('settingsAi.semantic.indexWaiting')} />
    )
  }

  if (status.state === 'failed') {
    return (
      <p className="text-[13px] text-destructive">
        {t('settingsAi.semantic.indexFailed')}
      </p>
    )
  }
  if (status.state === 'degraded') {
    return (
      <p className="text-[13px] text-destructive">
        {t('settingsAi.semantic.indexIncomplete')}
      </p>
    )
  }

  return (
    <p className="text-[13px] text-muted-foreground">
      {status.indexedItems > 0
        ? [
            t('settingsAi.semantic.indexed', { count: status.indexedItems }),
            status.lastIndexedAt &&
              t('settingsAi.semantic.lastIndexed', {
                when: format.relative(status.lastIndexedAt),
              }),
          ]
            .filter(Boolean)
            .join(' · ')
        : t('settingsAi.semantic.nothingIndexed')}
    </p>
  )
}

function ProgressLine({
  label,
  detail,
  value,
  action,
}: {
  label: string
  detail?: string
  /** 0–1; leave out while the total is not known yet. */
  value?: number
  action?: React.ReactNode
}) {
  return (
    <div role="status" className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span>{label}</span>
        <span className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
          {detail}
          {action}
        </span>
      </div>
      <Progress
        value={value === undefined ? undefined : Math.round(value * 100)}
        aria-label={label}
        className={value === undefined ? 'animate-pulse' : undefined}
      />
    </div>
  )
}
