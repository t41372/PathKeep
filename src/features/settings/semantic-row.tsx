/**
 * Semantic search: the switch, which model makes the index (the in-app one,
 * downloaded once, or an embeddings service the user adds), and the in-app
 * model's download. The chosen model's index under them is
 * `semantic-index-state.tsx`. Behavior comes from `useSemanticIndex` and
 * `useAiProviders`; this file only renders it.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  SwitchModelDialog,
  type SwitchTarget,
} from '@/features/ai-setup/model-switch'
import { ProviderDialog } from '@/features/ai-setup/provider-dialog'
import { isLocalProvider, providerLabel } from '@/features/ai-setup/providers'
import { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import {
  STATIC_MODEL_APPROX_BYTES,
  type useSemanticIndex,
} from '@/features/ai-setup/use-semantic-index'
import { useFormat, useI18n } from '@/lib/i18n'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '@/lib/types'
import { ProviderDetails } from './provider-details'
import { AutoUpdate, IndexState, ProgressLine } from './semantic-index-state'
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

/**
 * Which model makes the index, and the chosen service's own panel. Another
 * model is chosen only after the user has read what the switch does.
 */
function ModelPicker({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const providers = useAiProviders('embedding')
  const [adding, setAdding] = useState(false)
  const [switching, setSwitching] = useState<{
    id: string
    name: string
    target: SwitchTarget
  } | null>(null)

  const choose = (value: string) => {
    if (value === ADD) return setAdding(true)
    if (value === index.modelId) return
    if (value === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID) {
      return setSwitching({
        id: value,
        name: t('settingsAi.semantic.switch.inAppName'),
        target: {
          providerId: value,
          model:
            index.status.staticEmbedding?.modelRepo ??
            'potion-multilingual-128M',
          service: null,
        },
      })
    }
    const service = index.services.find((item) => item.id === value)
    if (!service) return
    setSwitching({
      id: value,
      name: service.name,
      target: {
        providerId: value,
        model: service.defaultModel,
        service: { name: service.name, local: isLocalProvider(service) },
      },
    })
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
      {index.service ? (
        <ProviderDetails
          provider={index.service}
          providers={providers}
          // The new model needs its own index; the row offers to build it.
          onModelChanged={() => void index.chooseModel(index.modelId)}
          hasIndex={index.status.indexedItems > 0}
        />
      ) : (
        <InAppModel index={index} />
      )}
      <SwitchModelDialog
        target={switching?.target ?? null}
        name={switching?.name ?? ''}
        onCancel={() => setSwitching(null)}
        onConfirm={() => {
          if (switching) void index.chooseModel(switching.id)
          setSwitching(null)
        }}
      />
      <ProviderDialog
        open={adding}
        onOpenChange={setAdding}
        purpose="embedding"
        onSave={async (draft) => {
          const service = await providers.add(draft)
          toast.success(t('settingsAi.provider.saved', { name: service.name }))
          // Chosen, not built: the row offers to build its index.
          void index.chooseModel(service.id)
        }}
      />
    </div>
  )
}

/** Which model the app comes with: what the index is made of, and why another means a rebuild. */
function InAppModel({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const status = index.status.staticEmbedding
  if (!status) return null
  return (
    <p className="text-xs text-muted-foreground">
      {t('settingsAi.semantic.model.inAppAbout', {
        model: status.modelRepo.split('/').at(-1) ?? status.modelRepo,
        dims: status.dimensions || 256,
      })}
    </p>
  )
}

function SemanticStatus({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const format = useFormat()
  const { download } = index

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

  return (
    <div className="flex flex-col gap-3">
      <IndexState index={index} />
      {index.error && (
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
          {t('settingsAi.semantic.actionFailed', { message: index.error })}
        </p>
      )}
      <AutoUpdate index={index} />
    </div>
  )
}
