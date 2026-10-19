/**
 * Which AI service the assistant uses, plus add / edit / test / remove for
 * it. The actions come from `useAiProviders`; the form is the shared
 * `ProviderDialog`, and the panel under the choice is `ProviderDetails`.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { ProviderDialog } from '@/features/ai-setup/provider-dialog'
import {
  providerLabel,
  selectedLlmProvider,
} from '@/features/ai-setup/providers'
import { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { ProviderDetails } from './provider-details'
import { RowSelect } from './row-select'

const NONE = '__none'
const ADD = '__add'

export function ProviderRow() {
  const { t } = useI18n()
  const ai = useSnapshot().config.ai
  const providers = useAiProviders()
  const [adding, setAdding] = useState(false)
  const selected = selectedLlmProvider(ai)

  const choose = (value: string) => {
    if (value === ADD) return setAdding(true)
    void providers.select(value === NONE ? null : value).catch((error) =>
      toast.error(
        t('settingsAi.provider.selectFailed', {
          message: describeError(error),
        }),
      ),
    )
  }

  return (
    <SettingRow
      title={t('settingsAi.provider.title')}
      description={t('settingsAi.provider.description')}
      htmlFor="settings-provider"
      control={
        <RowSelect
          id="settings-provider"
          value={selected?.id ?? NONE}
          disabled={providers.saving}
          onChange={choose}
          options={[
            { value: NONE, label: t('settingsAi.provider.none') },
            ...ai.llmProviders.map((provider) => ({
              value: provider.id,
              label: providerLabel(provider),
            })),
            { value: ADD, label: t('settingsAi.provider.add') },
          ]}
        />
      }
    >
      {selected && (
        <ProviderDetails provider={selected} providers={providers} />
      )}
      <ProviderDialog
        open={adding}
        onOpenChange={setAdding}
        onSave={async (draft) => {
          const provider = await providers.add(draft)
          toast.success(t('settingsAi.provider.saved', { name: provider.name }))
        }}
      />
    </SettingRow>
  )
}
