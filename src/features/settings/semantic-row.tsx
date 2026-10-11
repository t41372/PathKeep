/**
 * Semantic search: the switch, which model makes the index (the in-app one,
 * downloaded once, or an embeddings service the user adds), and the index
 * build, each with live progress. Behavior comes from `useSemanticIndex` and
 * `useAiProviders`; this file only renders it.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'
import { ProviderDialog } from '@/features/ai-setup/provider-dialog'
import { providerLabel } from '@/features/ai-setup/providers'
import { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import {
  STATIC_MODEL_APPROX_BYTES,
  type useSemanticIndex,
} from '@/features/ai-setup/use-semantic-index'
import { useFormat, useI18n } from '@/lib/i18n'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '@/lib/types'
import { ProviderDetails } from './provider-details'
import { RowSelect } from './row-select'

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
      description={
        index.service
          ? t('settingsAi.semantic.descriptionService', {
              name: index.service.name,
            })
          : t('settingsAi.semantic.description', { size: format.bytes(size) })
      }
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
      <ModelPicker index={index} />
      <SemanticStatus index={index} />
    </SettingRow>
  )
}

const ADD = '__add'

/** Which model makes the index, and the chosen service's own panel. */
function ModelPicker({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const providers = useAiProviders('embedding')
  const [adding, setAdding] = useState(false)

  const choose = (value: string) => {
    if (value === ADD) return setAdding(true)
    if (value !== index.modelId) void index.chooseModel(value)
  }

  return (
    <div className="flex flex-col gap-2 text-[13px]">
      <div className="flex items-center justify-between gap-3">
        <Label htmlFor="settings-semantic-model" className="font-normal">
          {t('settingsAi.semantic.model.label')}
        </Label>
        <RowSelect
          id="settings-semantic-model"
          value={index.modelId}
          disabled={index.saving || index.setupRunning || providers.saving}
          onChange={choose}
          options={[
            {
              value: BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
              label: t('settingsAi.semantic.model.inApp'),
            },
            ...index.services.map((service) => ({
              value: service.id,
              label: providerLabel(service),
            })),
            { value: ADD, label: t('settingsAi.semantic.model.add') },
          ]}
        />
      </div>
      {index.service && (
        <ProviderDetails provider={index.service} providers={providers} />
      )}
      <ProviderDialog
        open={adding}
        onOpenChange={setAdding}
        purpose="embedding"
        onSave={async (draft) => {
          const service = await providers.add(draft)
          toast.success(t('settingsAi.provider.saved', { name: service.name }))
          // Its index is built now if semantic search is on.
          void index.chooseModel(service.id)
        }}
      />
    </div>
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
    return !index.service && index.modelDownloaded ? (
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
      />
    </div>
  )
}
