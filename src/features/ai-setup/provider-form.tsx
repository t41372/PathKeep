/**
 * Fields for describing one AI service: kind, name, address, model, API key.
 * Used by the add and edit dialog in Settings, for the assistant and for
 * semantic search's embeddings, and inline in onboarding.
 */
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useI18n } from '@/lib/i18n'
import type { AiProviderPurpose } from '@/lib/types'
import {
  draftNeedsKey,
  presetFor,
  presetsFor,
  type ProviderDraft,
  type ProviderKind,
} from './providers'

export function ProviderForm({
  draft,
  onChange,
  idPrefix,
  purpose,
  keySaved = false,
}: {
  draft: ProviderDraft
  onChange: (draft: ProviderDraft) => void
  idPrefix: string
  purpose?: AiProviderPurpose
  /** Editing a service whose key is stored: an empty field keeps it. */
  keySaved?: boolean
}) {
  const { t } = useI18n()
  const preset = presetFor(draft.kind, purpose)
  const needsKey = draftNeedsKey(draft, purpose)
  const set = (patch: Partial<ProviderDraft>) =>
    onChange({ ...draft, ...patch })

  const pickKind = (kind: ProviderKind) =>
    set({
      kind,
      baseUrl: presetFor(kind, purpose).baseUrl,
      model: '',
      apiKey: '',
    })

  return (
    <div className="flex flex-col gap-3.5">
      <FormField
        id={`${idPrefix}-kind`}
        label={t('settingsAi.provider.form.kind')}
      >
        <Select
          value={draft.kind}
          onValueChange={(value) => pickKind(value as ProviderKind)}
        >
          <SelectTrigger id={`${idPrefix}-kind`} className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {presetsFor(purpose).map((item) => (
              <SelectItem key={item.kind} value={item.kind}>
                {t(`settingsAi.provider.kinds.${item.kind}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </FormField>
      <FormField
        id={`${idPrefix}-baseurl`}
        label={t('settingsAi.provider.form.baseUrl')}
      >
        <Input
          id={`${idPrefix}-baseurl`}
          value={draft.baseUrl}
          placeholder={preset.baseUrlHint}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => set({ baseUrl: event.target.value })}
        />
      </FormField>
      <FormField
        id={`${idPrefix}-model`}
        label={t('settingsAi.provider.form.model')}
      >
        <Input
          id={`${idPrefix}-model`}
          value={draft.model}
          placeholder={
            preset.modelHint
              ? t('settingsAi.provider.form.modelPlaceholder', {
                  example: preset.modelHint,
                })
              : undefined
          }
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => set({ model: event.target.value })}
        />
      </FormField>
      <FormField
        id={`${idPrefix}-name`}
        label={t('settingsAi.provider.form.name')}
      >
        <Input
          id={`${idPrefix}-name`}
          value={draft.name}
          placeholder={t('settingsAi.provider.form.namePlaceholder')}
          onChange={(event) => set({ name: event.target.value })}
        />
      </FormField>
      <FormField
        id={`${idPrefix}-key`}
        label={t('settingsAi.provider.form.apiKey')}
        hint={
          keySaved
            ? t('settingsAi.provider.form.apiKeyKeep')
            : needsKey
              ? t('settingsAi.provider.form.apiKeyHint')
              : t('settingsAi.provider.form.apiKeyOptional')
        }
      >
        <Input
          id={`${idPrefix}-key`}
          type="password"
          value={draft.apiKey}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => set({ apiKey: event.target.value })}
        />
      </FormField>
    </div>
  )
}

function FormField({
  id,
  label,
  hint,
  children,
}: {
  id: string
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  )
}
