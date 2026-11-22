/**
 * Semantic search: the switch, which model makes the index (the in-app one,
 * downloaded once, or an embeddings service the user adds), the in-app
 * model's download, and the index state under it. Mirrors
 * src/features/settings/semantic-row.tsx on the fake state.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { SwitchModelDialog, type SwitchTarget } from './model-switch'
import { ProviderDetails } from './provider-details'
import { ProviderDialog } from './provider-dialog'
import { appBytes, draftToProvider, isLocalProvider, providerLabel, STATIC_MODEL_APPROX_BYTES } from './providers'
import { RowSelect } from './row-select'
import { AutoUpdate, IndexState, ProgressLine } from './semantic-index-state'
import { IN_APP, IN_APP_MODEL, useAi, type AiState } from './state'

/** After the chosen model changes: its index has to be built; the complete one stays in use meanwhile. */
function chooseModel(s: AiState, id: string): Partial<AiState> {
  const enabled = s.phase !== 'off'
  const completeId = s.chosenComplete ? s.modelId : s.servingOther
  if (completeId === id) return { modelId: id, servingOther: null, chosenComplete: true, phase: enabled ? 'ready' : 'off', done: 0 }
  return { modelId: id, servingOther: completeId, chosenComplete: false, phase: enabled ? 'needsBuild' : 'off', done: 0 }
}

export function SemanticRow() {
  const ai = useAi()
  const on = ai.phase !== 'off'
  const enable = () =>
    ai.set((s) => (s.modelId === IN_APP && !s.modelDownloaded ? { phase: 'downloading', downloaded: 0 } : s.chosenComplete ? { phase: 'ready' } : { phase: 'running' }))

  return (
    <SettingRow
      title="語意搜尋"
      description={ai.service ? `依意思而不只是關鍵字來尋找頁面。由 ${ai.service.name} 建立索引。` : `依含義而不只是關鍵字來尋找頁面。模型在這台電腦上執行，只需下載一次，約 ${appBytes(STATIC_MODEL_APPROX_BYTES)}。`}
      htmlFor="settings-semantic"
      control={<Switch id="settings-semantic" checked={on} onCheckedChange={(next) => (next ? enable() : ai.set({ phase: 'off' }))} />}
    >
      <ModelPicker />
      <SemanticStatus onEnable={enable} />
    </SettingRow>
  )
}

const ADD = '__add'

function ModelPicker() {
  const ai = useAi()
  const [adding, setAdding] = useState(false)
  const [switching, setSwitching] = useState<{ id: string; name: string; target: SwitchTarget } | null>(null)
  const setupRunning = ai.phase === 'downloading' || ai.phase === 'setupRunning'

  const choose = (value: string) => {
    if (value === ADD) return setAdding(true)
    if (value === ai.modelId) return
    if (value === IN_APP) return setSwitching({ id: value, name: 'App 內建靜態模型', target: { providerId: value, model: IN_APP_MODEL, service: null } })
    const service = ai.embeddingServices.find((p) => p.id === value)
    if (!service) return
    setSwitching({ id: value, name: service.name, target: { providerId: value, model: service.defaultModel, service: { name: service.name, local: isLocalProvider(service) } } })
  }

  return (
    <div className="flex flex-col gap-2 text-[13px]">
      <div className="flex items-center justify-between gap-3">
        <Label htmlFor="settings-semantic-model" className="font-normal">
          模型
        </Label>
        <RowSelect
          id="settings-semantic-model"
          value={ai.modelId}
          disabled={setupRunning}
          onChange={choose}
          options={[{ value: IN_APP, label: 'App 內建靜態模型' }, ...ai.embeddingServices.map((p) => ({ value: p.id, label: providerLabel(p) })), { value: ADD, label: '新增嵌入服務…' }]}
        />
      </div>
      {ai.service ? (
        <ProviderDetails
          key={ai.service.id}
          provider={ai.service}
          purpose="embedding"
          hasIndex={ai.chosenComplete || ai.done > 0}
          onUpdate={(draft) => {
            const id = ai.service!.id
            ai.set((s) => {
              const before = s.embeddingServices.find((p) => p.id === id)!
              const updated = { ...draftToProvider(draft, id, 'embedding'), apiKeySaved: before.apiKeySaved || draft.apiKey.trim() !== '' }
              const services = s.embeddingServices.map((p) => (p.id === id ? updated : p))
              // Another model needs its own index; the row offers to build it.
              return updated.defaultModel !== before.defaultModel ? { embeddingServices: services, ...chooseModel({ ...s, embeddingServices: services }, id) } : { embeddingServices: services }
            })
          }}
          onRemove={() => {
            const id = ai.service!.id
            ai.set((s) => ({ embeddingServices: s.embeddingServices.filter((p) => p.id !== id), ...chooseModel(s, IN_APP), servingOther: null }))
          }}
        />
      ) : (
        <p className="text-xs text-muted-foreground">
          {IN_APP_MODEL}，256 維。隨 PathKeep 提供，在這台電腦上執行。它是靜態嵌入（static embedding）模型：同一個字詞不論出現在哪裡，對它來說意思都一樣，所以分不出「蘋果」是水果還是公司。因此它非常快，但找意思相近的頁面不如一般的上下文嵌入（contextual
          embedding）模型，那種模型會看每個字詞的前後文。想用的話，新增一個嵌入服務，例如 LM Studio 或 Ollama。
        </p>
      )}
      <SwitchModelDialog
        target={switching?.target ?? null}
        name={switching?.name ?? ''}
        onCancel={() => setSwitching(null)}
        onConfirm={() => {
          if (switching) ai.set((s) => chooseModel(s, switching.id))
          setSwitching(null)
        }}
      />
      <ProviderDialog
        open={adding}
        onOpenChange={setAdding}
        purpose="embedding"
        onSave={(draft) => {
          const service = draftToProvider(draft, `embed-${draft.kind}-${Date.now().toString(36)}`, 'embedding')
          // Chosen, not built: the row offers to build its index.
          ai.set((s) => ({ embeddingServices: [...s.embeddingServices, service], ...chooseModel(s, service.id) }))
          toast.success(`${service.name} 已就緒。`)
          ai.check(service.id)
        }}
      />
    </div>
  )
}

function SemanticStatus({ onEnable }: { onEnable: () => void }) {
  const ai = useAi()
  if (ai.phase === 'downloading') {
    return (
      <ProgressLine
        label="正在下載語言模型…"
        detail={`${appBytes(ai.downloaded)} / ${appBytes(STATIC_MODEL_APPROX_BYTES)}`}
        value={ai.downloaded / STATIC_MODEL_APPROX_BYTES}
        action={
          <Button size="xs" variant="ghost" onClick={() => ai.set({ phase: 'modelMissing' })}>
            取消
          </Button>
        }
      />
    )
  }
  if (ai.phase === 'off') return !ai.service && ai.modelDownloaded ? <p className="text-[13px] text-muted-foreground">關閉後會保留已下載的模型。</p> : null
  if (ai.phase === 'downloadFailed') {
    return (
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span className="text-destructive">下載沒有完成。error decoding response body: connection reset by peer</span>
        <Button size="xs" variant="outline" onClick={onEnable}>
          重試
        </Button>
      </div>
    )
  }
  if (ai.phase === 'modelMissing') {
    return (
      <div className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
        模型還沒有下載。
        <Button size="xs" variant="outline" onClick={() => ai.set({ phase: 'downloading', downloaded: 0 })}>
          下載
        </Button>
      </div>
    )
  }
  return (
    <div className="flex flex-col gap-3">
      <IndexState />
      <AutoUpdate />
    </div>
  )
}
