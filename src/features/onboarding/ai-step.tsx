/**
 * Step 6: optional AI. Not now, local semantic search, or an AI service.
 * Reuses the provider form from Settings → AI; the choice is carried out on
 * the last step, after the first backup, because the index needs an archive.
 */
import { CircleOff, Cpu, Plug } from 'lucide-react'
import { ProviderForm } from '@/features/ai-setup/provider-form'
import { isLocalUrl } from '@/features/ai-setup/providers'
import { STATIC_MODEL_APPROX_BYTES } from '@/features/ai-setup/use-semantic-index'
import { useFormat, useI18n } from '@/lib/i18n'
import { ChoiceCard, ChoiceGroup } from './choice-card'
import type { AiChoice, Draft } from './draft'

const icons = { off: CircleOff, local: Cpu, provider: Plug }

export function AiStep({
  draft,
  where,
  onChange,
}: {
  draft: Draft
  where: string
  onChange: (patch: Partial<Draft>) => void
}) {
  const { t } = useI18n()
  const format = useFormat()

  return (
    <>
      <ChoiceGroup
        value={draft.ai}
        onValueChange={(ai: AiChoice) => onChange({ ai })}
        label={t('onboarding.steps.ai.label')}
      >
        {(['off', 'local', 'provider'] as const).map((choice) => {
          const Icon = icons[choice]
          return (
            <ChoiceCard
              key={choice}
              value={choice}
              leading={
                <Icon
                  className="mt-px size-[18px] shrink-0"
                  strokeWidth={1.75}
                />
              }
              title={t(`onboarding.ai.${choice}.title`)}
              body={t(`onboarding.ai.${choice}.body`, { where })}
              trailing={
                choice === 'local' && (
                  <span className="font-mono text-xs whitespace-nowrap text-muted-foreground">
                    {t('onboarding.ai.approxSize', {
                      size: format.bytes(STATIC_MODEL_APPROX_BYTES),
                    })}
                  </span>
                )
              }
            />
          )
        })}
      </ChoiceGroup>

      {draft.ai === 'provider' && (
        <section className="flex animate-rise flex-col gap-3 rounded-xl border bg-card p-4 shadow-card">
          <ProviderForm
            draft={draft.provider}
            onChange={(provider) => onChange({ provider })}
            idPrefix="onboarding-provider"
          />
          <p className="text-xs text-muted-foreground">
            {isLocalUrl(draft.provider.baseUrl)
              ? t('settingsAi.provider.localNote')
              : t('settingsAi.provider.remoteNote')}
          </p>
        </section>
      )}

      <p className="text-[13px] text-muted-foreground">
        {t('onboarding.ai.note')}
      </p>
    </>
  )
}
