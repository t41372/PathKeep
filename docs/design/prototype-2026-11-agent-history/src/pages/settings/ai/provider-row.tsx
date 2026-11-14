/**
 * Which AI service the assistant uses, plus add / edit / test / remove, as
 * src/features/settings/provider-row.tsx.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { ProviderDetails } from './provider-details'
import { ProviderDialog } from './provider-dialog'
import { draftToProvider, providerLabel } from './providers'
import { RowSelect } from './row-select'
import { useAi } from './state'

const NONE = '__none'
const ADD = '__add'

export function ProviderRow() {
  const ai = useAi()
  const [adding, setAdding] = useState(false)
  const selected = ai.llmServices.find((p) => p.id === ai.llmSelected) ?? null

  return (
    <SettingRow
      title="AI 服務"
      description="助理使用的模型。"
      htmlFor="settings-provider"
      control={
        <RowSelect
          id="settings-provider"
          value={selected?.id ?? NONE}
          onChange={(value) => (value === ADD ? setAdding(true) : ai.set({ llmSelected: value === NONE ? null : value }))}
          options={[{ value: NONE, label: '無' }, ...ai.llmServices.map((p) => ({ value: p.id, label: providerLabel(p) })), { value: ADD, label: '新增服務…' }]}
        />
      }
    >
      {selected && (
        <ProviderDetails
          key={selected.id}
          provider={selected}
          purpose="llm"
          onUpdate={(draft) => ai.set((s) => ({ llmServices: s.llmServices.map((p) => (p.id === selected.id ? { ...draftToProvider(draft, p.id), apiKeySaved: p.apiKeySaved || draft.apiKey.trim() !== '' } : p)) }))}
          onRemove={() => ai.set((s) => ({ llmServices: s.llmServices.filter((p) => p.id !== selected.id), llmSelected: null }))}
        />
      )}
      <ProviderDialog
        open={adding}
        onOpenChange={setAdding}
        onSave={(draft) => {
          const provider = draftToProvider(draft, `llm-${draft.kind}-${Date.now().toString(36)}`)
          ai.set((s) => ({ llmServices: [...s.llmServices, provider], llmSelected: provider.id }))
          toast.success(`${provider.name} 已就緒。`)
          ai.check(provider.id)
        }}
      />
    </SettingRow>
  )
}
