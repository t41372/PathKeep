/**
 * Fields for one AI service (kind, address, model, name, API key), as
 * src/features/ai-setup/provider-form.tsx. Used by the add and edit dialog in
 * Settings › AI and inline in onboarding's AI step.
 */
import type { ReactNode } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ModelField, type ModelListCase } from './model-field'
import { draftNeedsKey, kindLabels, presetFor, presetsFor, type ProviderDraft, type ProviderKind, type Purpose } from './providers'

export function ProviderForm({
  draft,
  onChange,
  idPrefix,
  purpose,
  keySaved = false,
  listCase,
}: {
  draft: ProviderDraft
  onChange: (draft: ProviderDraft) => void
  idPrefix: string
  purpose?: Purpose
  /** Editing a service whose key is stored: an empty field keeps it. */
  keySaved?: boolean
  listCase: ModelListCase
}) {
  const preset = presetFor(draft.kind, purpose)
  const needsKey = draftNeedsKey(draft, purpose)
  const set = (patch: Partial<ProviderDraft>) => onChange({ ...draft, ...patch })
  const pickKind = (kind: ProviderKind) => set({ kind, baseUrl: presetFor(kind, purpose).baseUrl, model: '', apiKey: '' })

  return (
    <div className="flex flex-col gap-3.5">
      <FormField id={`${idPrefix}-kind`} label="服務">
        <Select value={draft.kind} onValueChange={(value) => pickKind(value as ProviderKind)}>
          <SelectTrigger id={`${idPrefix}-kind`} className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {presetsFor(purpose).map((item) => (
              <SelectItem key={item.kind} value={item.kind}>
                {kindLabels[item.kind]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </FormField>
      <FormField id={`${idPrefix}-baseurl`} label="位址">
        <Input id={`${idPrefix}-baseurl`} value={draft.baseUrl} placeholder={preset.baseUrlHint} spellCheck={false} autoComplete="off" onChange={(event) => set({ baseUrl: event.target.value })} />
      </FormField>
      <FormField id={`${idPrefix}-model`} label="模型">
        <ModelField
          id={`${idPrefix}-model`}
          draft={draft}
          purpose={purpose}
          keySaved={keySaved}
          listCase={listCase}
          placeholder={preset.modelHint ? `例如 ${preset.modelHint}` : undefined}
          onChange={(model) => set({ model })}
        />
      </FormField>
      <FormField id={`${idPrefix}-name`} label="名稱">
        <Input id={`${idPrefix}-name`} value={draft.name} placeholder="選填" onChange={(event) => set({ name: event.target.value })} />
      </FormField>
      <FormField
        id={`${idPrefix}-key`}
        label="API 金鑰"
        hint={
          keySaved
            ? '留空就保留已儲存的金鑰。'
            : needsKey
              ? '儲存在系統鑰匙圈裡，不會寫進 PathKeep 的設定檔。'
              : '可不填：只有這台電腦上要求金鑰的伺服器才需要。金鑰儲存在系統鑰匙圈裡。'
        }
      >
        <Input id={`${idPrefix}-key`} type="password" value={draft.apiKey} autoComplete="off" spellCheck={false} onChange={(event) => set({ apiKey: event.target.value })} />
      </FormField>
    </div>
  )
}

function FormField({ id, label, hint, children }: { id: string; label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  )
}
